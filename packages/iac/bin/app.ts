#!/usr/bin/env node
import 'source-map-support/register';
import * as cdk from 'aws-cdk-lib';
import { UntrustedCodeStack } from '../lib/untrusted-code-stack';

const app = new cdk.App();
new UntrustedCodeStack(app, 'UntrustedCodeStack', {
  env: {
    account: process.env.CDK_DEFAULT_ACCOUNT,
    region: process.env.CDK_DEFAULT_REGION,
  },
  description: 'Lambda function for executing untrusted JavaScript code using QuickJS',
});
