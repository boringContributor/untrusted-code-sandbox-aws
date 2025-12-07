import * as cdk from 'aws-cdk-lib';
import * as lambda from 'aws-cdk-lib/aws-lambda';
import * as logs from 'aws-cdk-lib/aws-logs';
import { Construct } from 'constructs';
import * as path from 'path';

export class UntrustedCodeStack extends cdk.Stack {
  public readonly lambdaFunction: lambda.Function;

  constructor(scope: Construct, id: string, props?: cdk.StackProps) {
    super(scope, id, props);

    // Lambda function for executing untrusted JavaScript code
    this.lambdaFunction = new lambda.Function(this, 'JsExecutorFunction', {
      runtime: lambda.Runtime.PROVIDED_AL2023,
      handler: 'bootstrap',
      code: lambda.Code.fromAsset(path.join(__dirname, '../../lambda/target/lambda/bootstrap')),
      memorySize: 512,
      timeout: cdk.Duration.seconds(30),
      architecture: lambda.Architecture.ARM_64,
      environment: {
        RUST_BACKTRACE: '1',
        RUST_LOG: 'info',
      },
      logRetention: logs.RetentionDays.ONE_WEEK,
      description: 'Executes untrusted JavaScript code in a QuickJS sandbox',
      reservedConcurrentExecutions: 100, // Limit concurrent executions for safety
    });

    // Output the function ARN and name
    new cdk.CfnOutput(this, 'FunctionArn', {
      value: this.lambdaFunction.functionArn,
      description: 'ARN of the JS executor Lambda function',
      exportName: 'JsExecutorFunctionArn',
    });

    new cdk.CfnOutput(this, 'FunctionName', {
      value: this.lambdaFunction.functionName,
      description: 'Name of the JS executor Lambda function',
      exportName: 'JsExecutorFunctionName',
    });
  }
}
