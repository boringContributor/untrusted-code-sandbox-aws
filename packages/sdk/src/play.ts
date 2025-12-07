import { UntrustedCodeClient } from "./client";

// To run this, you need to:
// 1. Deploy the Lambda: npm run deploy (from root)
// 2. Set LAMBDA_FUNCTION_NAME env var
// 3. Run: LAMBDA_FUNCTION_NAME=your-function-name npx tsx src/play.ts

const functionName = 'UntrustedCodeStack-JsExecutorFunction2DB0BFF6-ktaXy6kUnG5s'

if (!functionName) {
  console.error('❌ Error: LAMBDA_FUNCTION_NAME environment variable is required');
  console.log('\nTo use this script:');
  console.log('1. Deploy the Lambda:');
  console.log('   npm run deploy');
  console.log('\n2. Get the function name from the output, then run:');
  console.log('   LAMBDA_FUNCTION_NAME=UntrustedCodeStack-JsExecutorFunction npx tsx src/play.ts');
  console.log('\nOr set it in your shell:');
  console.log('   export LAMBDA_FUNCTION_NAME=UntrustedCodeStack-JsExecutorFunction');
  process.exit(1);
}

const client = new UntrustedCodeClient({
  functionName,
  region: process.env.AWS_REGION || 'eu-central-1',
});

// Demo: Sending data to webhook.site

async function runWebhookExample() {
  console.log('🚀 Sending data to webhook.site...\n');

  const webhookCode = `
    console.log('Sending data to webhook...');

    const response = fetch('https://webhook.site/d7b83a74-fb33-4798-9060-47288b9b0574', { method: 'POST', body: JSON.stringify({ message: 'Hello from untrusted code!' }) });

    if (!response.ok) {
      ({ error: response.error, status: response.status })
    } else {
      console.log('Webhook received! Status:', response.status);
      ({
        success: true,
        status: response.status,
        message: 'Data sent to webhook successfully'
      })
    }
  `;

  try {
    const result = await client.runUntrustedCode({
      code: webhookCode,
      allowedDomains: ['webhook.site']
    });
    console.log('✅ Success:', result.success);
    console.log('📦 Result:', JSON.stringify(result.result, null, 2));
    if (result.consoleOutput.length > 0) {
      console.log('📝 Console:', result.consoleOutput.join('\n'));
    }
    console.log('\n💡 Check your webhook at: https://webhook.site/#!/d7b83a74-fb33-4798-9060-47288b9b0574');
  } catch (error: any) {
    console.error('❌ Error:', error.message);
  }
}

runWebhookExample();