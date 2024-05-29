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
    vercel = {
      source  = "vercel/vercel"
      version = "~> 1.11"
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

data "terraform_remote_state" "project" {
  backend   = "s3"
  workspace = "default"

  config = {
    bucket         = "dglsparsons-terraform-state"
    key            = "pipedream-project.tfstate"
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

provider "vercel" {
  team = "stygian-software"
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

resource "vercel_project_environment_variable" "dynamodb_workflows" {
  project_id = data.terraform_remote_state.project.outputs.vercel_project_id
  key        = "DYNAMODB_WORKFLOWS"
  value      = aws_dynamodb_table.workflows.name
  target     = ["production", "preview"]
}

resource "vercel_project_environment_variable" "github_client_id" {
  project_id = data.terraform_remote_state.project.outputs.vercel_project_id
  key        = "GITHUB_CLIENT_ID"
  value      = "Iv1.a37bc6120f071efe"
  target     = ["production", "preview"]
}

data "aws_ssm_parameter" "github_client_secret" {
  name = "/${local.prefix}/github_client_secret"
}

resource "vercel_project_environment_variable" "github_client_secret" {
  project_id = data.terraform_remote_state.project.outputs.vercel_project_id
  key        = "GITHUB_CLIENT_SECRET"
  value      = data.aws_ssm_parameter.github_client_secret.value
  target     = ["production", "preview"]
  sensitive  = true
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

resource "aws_iam_access_key" "pipedream_unencrypted" {
  user = aws_iam_user.pipedream.name
}

resource "vercel_project_environment_variable" "aws_access_key" {
  project_id = data.terraform_remote_state.project.outputs.vercel_project_id
  key        = "PIPEDREAM_AWS_ACCESS_KEY_ID"
  sensitive  = true
  value      = aws_iam_access_key.pipedream_unencrypted.id
  target     = ["production", "preview"]
}

resource "vercel_project_environment_variable" "aws_secret_access_key" {
  project_id = data.terraform_remote_state.project.outputs.vercel_project_id
  key        = "PIPEDREAM_AWS_SECRET_ACCESS_KEY"
  sensitive  = true
  value      = aws_iam_access_key.pipedream_unencrypted.secret
  target     = ["production", "preview"]
}

data "aws_region" "current" {}

resource "vercel_project_environment_variable" "aws_region" {
  project_id = data.terraform_remote_state.project.outputs.vercel_project_id
  key        = "PIPEDREAM_AWS_REGION"
  target     = ["production", "preview"]
  value      = data.aws_region.current.name
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
