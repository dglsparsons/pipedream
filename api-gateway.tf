resource "aws_apigatewayv2_api" "api" {
  name          = local.prefix
  protocol_type = "HTTP"
  route_key     = "$default"
  target        = aws_lambda_function.lambda.arn
}

resource "aws_lambda_permission" "api_permissions" {
  action        = "lambda:InvokeFunction"
  function_name = aws_lambda_function.lambda.function_name
  principal     = "apigateway.amazonaws.com"
  source_arn    = "${aws_apigatewayv2_api.api.execution_arn}/*/$default"
}

resource "aws_cloudwatch_metric_alarm" "api_5xx" {
  alarm_name          = "${local.prefix}-5XX"
  comparison_operator = "GreaterThanThreshold"
  evaluation_periods  = "1"
  metric_name         = "5xx"
  namespace           = "AWS/ApiGateway"
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
    ApiId = aws_apigatewayv2_api.api.id
  }
}

output "api_endpoint" {
  value = aws_apigatewayv2_api.api.api_endpoint
}
