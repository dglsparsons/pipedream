terraform {
  required_version = "1.6.2"
  backend "s3" {
    bucket         = "dglsparsons-terraform-state"
    key            = "pipedream.tfstate"
    region         = "eu-west-1"
    dynamodb_table = "terraform-lock"
  }

  required_providers {
    aws = {
      source  = "hashicorp/aws"
      version = "~> 5.25"
    }
  }
}

data "terraform_remote_state" "platform" {
  backend   = "s3"
  workspace = "default"

  config = {
    bucket         = "dglsparsons-terraform-state"
    key            = "platform-core.tfstate"
    region         = "eu-west-1"
    dynamodb_table = "terraform-lock"
  }
}

locals {
  environment = "doug"
  prefix      = "pipedream"
}

provider "aws" {
  region = "eu-west-1"

  assume_role {
    role_arn = "arn:aws:iam::${data.terraform_remote_state.platform.outputs.account_ids[local.environment]}:role/Deployer"
  }
}

resource "aws_dynamodb_table" "workflows" {
  name         = "${local.prefix}-workflows"
  billing_mode = "PAY_PER_REQUEST"
  hash_key     = "id" # composite of owner/repo
  range_key    = "created_at"

  attribute {
    name = "id"
    type = "S"
  }

  attribute {
    name = "created_at"
    type = "S"
  }

  attribute {
    name = "status"
    type = "S"
  }

  attribute {
    name = "due_to_run"
    type = "S"
  }

  global_secondary_index {
    hash_key        = "status"
    range_key       = "due_to_run"
    name            = "workflows_by_status"
    projection_type = "ALL"
  }
}

data "aws_iam_policy_document" "workflows_dynamodb" {
  statement {
    actions = [
      "dynamodb:Query",
      "dynamodb:GetItem",
      "dynamodb:PutItem",
      "dynamodb:UpdateItem",
      "dynamodb:ConditionCheckItem",
    ]
    resources = [
      aws_dynamodb_table.workflows.arn,
      "${aws_dynamodb_table.workflows.arn}/index/*",
    ]
  }
}

resource "aws_iam_policy" "workflows_dynamodb" {
  name   = "${local.prefix}-workflows-dynamodb"
  policy = data.aws_iam_policy_document.workflows_dynamodb.json
}


## Here on down is just for local dev
resource "aws_iam_user" "pipedream" {
  name          = "${local.prefix}-api"
  force_destroy = true
}

resource "aws_iam_user_policy_attachment" "workflows_dynamodb" {
  user       = aws_iam_user.pipedream.name
  policy_arn = aws_iam_policy.workflows_dynamodb.arn
}

resource "aws_iam_access_key" "pipedream" {
  user    = aws_iam_user.pipedream.name
  pgp_key = "keybase:dgls"
}

output "access_key" {
  value = aws_iam_access_key.pipedream.id
}

output "encrypted_secret_access_key" {
  value = aws_iam_access_key.pipedream.encrypted_secret
}

output "table_name" {
  value = aws_dynamodb_table.workflows.name
}
