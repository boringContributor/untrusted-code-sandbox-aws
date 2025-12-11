import { LambdaClient, InvokeCommand } from '@aws-sdk/client-lambda';
import { ExecuteRequest, ExecuteResponse, RunUntrustedCodeOptions, StructuredError } from './types';

/**
 * Execute untrusted JavaScript code in a secure AWS Lambda sandbox
 *
 * @param options - The execution options including code and sandbox settings
 * @returns Promise resolving to the execution response
 * @throws Error if Lambda invocation fails or function name is not provided
 *
 * @example
 * ```typescript
 * import { runUntrustedCode } from '@untrusted-code/sdk';
 *
 * // Simple calculation (uses SANDBOX_FUNCTION_NAME env var)
 * const result = await runUntrustedCode({
 *   code: '2 + 2'
 * });
 * console.log(result.result); // 4
 *
 * // With explicit function name
 * const result = await runUntrustedCode({
 *   code: '2 + 2',
 *   functionName: 'my-lambda-function'
 * });
 *
 * // With timeout and memory limits
 * const result = await runUntrustedCode({
 *   code: 'while(true) {}',
 *   timeoutMs: 1000,
 *   memoryLimitBytes: 10 * 1024 * 1024 // 10MB
 * });
 *
 * // With network access
 * const result = await runUntrustedCode({
 *   code: `
 *     const response = await fetch('https://api.example.com/data');
 *     return response.ok ? response.json() : null;
 *   `,
 *   allowedDomains: ['api.example.com']
 * });
 *
 * // With options parameter
 * const result = await runUntrustedCode({
 *   code: `
 *     // options parameter is automatically available
 *     const response = await fetch(options.apiUrl);
 *     const data = await response.json();
 *     return { userId: options.userId, data };
 *   `,
 *   options: {
 *     userId: '123',
 *     apiUrl: 'https://api.example.com/users/123'
 *   },
 *   allowedDomains: ['api.example.com']
 * });
 * ```
 */
export async function runUntrustedCode(
  options: RunUntrustedCodeOptions
): Promise<ExecuteResponse> {
  const functionName = options.functionName || process.env.SANDBOX_FUNCTION_NAME;

  if (!functionName) {
    throw new Error(
      'Function name must be provided via options.functionName or SANDBOX_FUNCTION_NAME environment variable'
    );
  }

  // Create Lambda client with region from options or AWS SDK defaults
  const lambdaClient = new LambdaClient(
    options.region ? { region: options.region } : {}
  );

  // Prepare the Lambda request payload
  const request: ExecuteRequest = {
    code: options.code,
    timeoutMs: options.timeoutMs,
    memoryLimitBytes: options.memoryLimitBytes,
    allowedDomains: options.allowedDomains,
    options: options.options,
  };

  const command = new InvokeCommand({
    FunctionName: functionName,
    Payload: JSON.stringify(request),
  });

  try {
    const response = await lambdaClient.send(command);

    if (!response.Payload) {
      throw new Error('No payload in Lambda response');
    }

    const result = JSON.parse(new TextDecoder().decode(response.Payload));

    // Check for Lambda-level errors
    if (response.FunctionError) {
      throw new Error(`Lambda error: ${response.FunctionError} - ${JSON.stringify(result)}`);
    }

    return result as ExecuteResponse;
  } catch (error) {
    if (error instanceof Error) {
      throw new Error(`Failed to invoke Lambda: ${error.message}`);
    }
    throw error;
  }
}

/**
 * Extract structured error information from an ExecuteResponse
 *
 * @param response - The response from runUntrustedCode
 * @returns StructuredError object with errorOutput and skipReason fields
 *
 * @example
 * ```typescript
 * const response = await runUntrustedCode({ code: 'return { skip_reason: "user_cancelled" }' });
 * const structured = getStructuredError(response);
 * if (structured.skipReason) {
 *   console.log('Execution was skipped:', structured.skipReason);
 * }
 * ```
 */
export function getStructuredError(response: ExecuteResponse): StructuredError {
  return {
    errorOutput: response.error_reason,
    skipReason: response.skip_reason,
  };
}
