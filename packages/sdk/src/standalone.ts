import { UntrustedCodeClient } from './client';
import { ExecuteRequest, ExecuteResponse } from './types';

/**
 * Standalone function to execute untrusted code
 *
 * This is a convenience function that creates a client instance for you.
 * Use UntrustedCodeClient directly if you need to reuse the client.
 *
 * @param request - The execution request
 * @param functionName - AWS Lambda function name (defaults to env var LAMBDA_FUNCTION_NAME)
 * @param region - AWS region (defaults to env var AWS_REGION or 'us-east-1')
 * @returns Promise resolving to the execution response
 *
 * @example
 * ```typescript
 * import { runUntrustedCode } from '@untrusted-code/sdk';
 *
 * const result = await runUntrustedCode(
 *   { code: '2 + 2' },
 *   'my-lambda-function',
 *   'us-east-1'
 * );
 * ```
 */
export async function runUntrustedCode(
  request: ExecuteRequest,
  functionName?: string,
  region?: string
): Promise<ExecuteResponse> {
  const fn = functionName || process.env.LAMBDA_FUNCTION_NAME;

  if (!fn) {
    throw new Error(
      'Function name must be provided or set via LAMBDA_FUNCTION_NAME environment variable'
    );
  }

  const client = new UntrustedCodeClient({
    functionName: fn,
    region,
  });

  return client.runUntrustedCode(request);
}
