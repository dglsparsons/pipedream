locals {
  function_name    = "pipedream"
  github_client_id = "Iv1.a37bc6120f071efe"
  parameter_access = [
    "pipedream/github_client_secret",
  ]
}

data "aws_arn" "lambda" {
  arn = aws_lambda_function.lambda.arn
}

resource "aws_cloudwatch_metric_alarm" "error" {
  alarm_name          = "${aws_lambda_function.lambda.function_name}-error"
  comparison_operator = "GreaterThanThreshold"
  evaluation_periods  = "1"
  metric_name         = "Errors"
  namespace           = "AWS/Lambda"
  period              = "60"
  statistic           = "Average"
  threshold           = 0
  alarm_description   = <<EOF
View the logs: https://${data.aws_arn.lambda.region}.console.aws.amazon.com/cloudwatch/home?region=${data.aws_arn.lambda.region}#logsV2:logs-insights$3FqueryDetail$3D$257E$2528end$257E0$257Estart$257E-3600$257EtimeType$257E$2527RELATIVE$257Eunit$257E$2527seconds$257EeditorString$257E$2527fields*20*40timestamp*2c*20*40message*0a*7c*20sort*20*40timestamp*20desc*0a*7c*20limit*2020$257EisLiveTail$257Efalse$257EqueryId$257E$2527e2708067-812c-47d3-a2d9-3a3a48ca0105$257Esource$257E$2528$257E$2527*2faws*2flambda*2f${aws_lambda_function.lambda.function_name}$2529$2529
  EOF
  alarm_actions = [
    "arn:aws:sns:${data.aws_arn.lambda.region}:${data.aws_arn.lambda.account}:${local.environment}-monitoring-alerts",
  ]
  treat_missing_data = "notBreaching"
  dimensions = {
    FunctionName = aws_lambda_function.lambda.function_name
  }
}

resource "aws_cloudwatch_log_group" "example" {
  name              = "/aws/lambda/${local.environment}-${local.function_name}"
  retention_in_days = 14
}

data "aws_iam_policy_document" "logging" {
  statement {
    actions = [
      "logs:PutLogEvents",
      "logs:CreateLogStream",
    ]

    resources = [
      "arn:aws:logs:*:*:log-group:/aws/lambda/${aws_iam_role.lambda.name}",
      "arn:aws:logs:*:*:log-group:/aws/lambda/${aws_iam_role.lambda.name}:*",
    ]
  }
}

resource "aws_iam_policy" "logging" {
  description = "Access to read/write all monitoring related stuff"
  name        = "${aws_iam_role.lambda.name}-logging"
  policy      = data.aws_iam_policy_document.logging.json
}

resource "aws_iam_role_policy_attachment" "logging" {
  role       = aws_iam_role.lambda.name
  policy_arn = aws_iam_policy.logging.arn
}

data "archive_file" "lambda" {
  type        = "zip"
  source_dir  = "target/lambda/${local.function_name}"
  output_path = "target/lambda-zip/${local.function_name}/bootstrap.zip"
}

resource "aws_iam_role" "lambda" {
  name        = "${local.environment}-${local.function_name}"
  description = "IAM for the ${local.function_name} lambda"

  assume_role_policy = <<EOF
{
  "Version": "2012-10-17",
  "Statement": [
    {
      "Action": "sts:AssumeRole",
      "Principal": {
        "Service": "lambda.amazonaws.com"
      },
      "Effect": "Allow",
      "Sid": ""
    }
  ]
}
EOF
}

resource "aws_lambda_function" "lambda" {
  function_name = "${local.environment}-${local.function_name}"
  role          = aws_iam_role.lambda.arn
  architectures = ["arm64"]

  filename         = data.archive_file.lambda.output_path
  source_code_hash = data.archive_file.lambda.output_base64sha256

  runtime = "provided.al2023"
  handler = "anything_works"

  memory_size = 128
  timeout     = 30

  environment {
    variables = {
      ENVIRONMENT        = local.environment
      FUNCTION_NAME      = local.function_name
      DYNAMODB_WORKFLOWS = aws_dynamodb_table.workflows.name
      GITHUB_CLIENT_ID   = local.github_client_id
      LEPTOS_SITE_ROOT   = "."
    }
  }
}

data "aws_iam_policy_document" "parameters" {
  statement {
    actions = [
      "ssm:GetParameter",
    ]

    resources = formatlist("arn:aws:ssm:*:*:parameter/%s", local.parameter_access)
  }
}

resource "aws_iam_policy" "parameters" {
  count  = length(local.parameter_access) > 0 ? 1 : 0
  name   = "${aws_iam_role.lambda.name}-parameters"
  policy = data.aws_iam_policy_document.parameters.json
}

resource "aws_iam_role_policy_attachment" "parameters" {
  count      = length(local.parameter_access) > 0 ? 1 : 0
  role       = aws_iam_role.lambda.name
  policy_arn = aws_iam_policy.parameters[0].arn
}

resource "aws_iam_role_policy_attachment" "dynamodb_write" {
  role       = aws_iam_role.lambda.name
  policy_arn = aws_iam_policy.workflows_dynamodb.arn
}
