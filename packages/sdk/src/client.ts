import { LambdaClient, InvokeCommand } from '@aws-sdk/client-lambda';
import { ExecuteRequest, ExecuteResponse, UntrustedCodeClientConfig } from './types';

/**
 * Client for executing untrusted JavaScript code in AWS Lambda sandbox
 *
 * @example
 * ```typescript
 * const client = new UntrustedCodeClient({
 *   functionName: 'my-lambda-function',
 *   region: 'us-east-1'
 * });
 *
 * const result = await client.runUntrustedCode({
 *   code: '2 + 2'
 * });
 *
 * console.log(result.result); // 4
 * ```
 */
export class UntrustedCodeClient {
  private lambdaClient: LambdaClient;
  private functionName: string;

  constructor(config: UntrustedCodeClientConfig) {
    this.functionName = config.functionName;

    if (config.lambdaClient) {
      this.lambdaClient = config.lambdaClient;
    } else {
      this.lambdaClient = new LambdaClient({
        region: config.region || process.env.AWS_REGION || 'us-east-1',
      });
    }
  }

  /**
   * Execute untrusted JavaScript code in a secure sandbox
   *
   * @param request - The execution request containing code and optional settings
   * @returns Promise resolving to the execution response
   * @throws Error if Lambda invocation fails
   *
   * @example
   * ```typescript
   * // Simple calculation
   * const result = await client.runUntrustedCode({
   *   code: '2 + 2'
   * });
   *
   * // With timeout
   * const result = await client.runUntrustedCode({
   *   code: 'while(true) {}',
   *   timeoutMs: 1000
   * });
   *
   * // Data transformation
   * const result = await client.runUntrustedCode({
   *   code: `
   *     const data = [1, 2, 3, 4, 5];
   *     data.filter(x => x > 2).map(x => x * 2)
   *   `
   * });
   *
   * // With network access
   * const result = await client.runUntrustedCode({
   *   code: `
   *     const response = fetch('https://api.example.com/data');
   *     response.ok ? response.json : null
   *   `,
   *   allowedDomains: ['api.example.com']
   * });
   * ```
   */
  async runUntrustedCode(request: ExecuteRequest): Promise<ExecuteResponse> {
    const command = new InvokeCommand({
      FunctionName: this.functionName,
      Payload: JSON.stringify(request),
    });

    try {
      const response = await this.lambdaClient.send(command);

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
   * Execute code with a custom timeout
   */
  async runWithTimeout(code: string, timeoutMs: number): Promise<ExecuteResponse> {
    return this.runUntrustedCode({ code, timeoutMs });
  }

  /**
   * Execute code with custom memory limit
   */
  async runWithMemoryLimit(code: string, memoryLimitBytes: number): Promise<ExecuteResponse> {
    return this.runUntrustedCode({ code, memoryLimitBytes });
  }

  /**
   * Execute code with network access to allowed domains
   *
   * @param code - The JavaScript code to execute
   * @param allowedDomains - List of domains that fetch() can access
   * @returns Promise resolving to the execution response
   *
   * @example
   * ```typescript
   * // Fetch data from an API
   * const result = await client.runWithNetworking(
   *   `
   *     const response = fetch('https://api.github.com/users/github');
   *     response.ok ? response.json : { error: response.error }
   *   `,
   *   ['api.github.com']
   * );
   * ```
   */
  async runWithNetworking(code: string, allowedDomains: string[]): Promise<ExecuteResponse> {
    return this.runUntrustedCode({ code, allowedDomains });
  }

  /**
   * Batch execute multiple code snippets in parallel
   *
   * @example
   * ```typescript
   * const results = await client.batchExecute([
   *   { code: '1 + 1' },
   *   { code: '2 * 2' },
   *   { code: '3 ** 3' }
   * ]);
   * ```
   */
  async batchExecute(requests: ExecuteRequest[]): Promise<ExecuteResponse[]> {
    return Promise.all(
      requests.map(request => this.runUntrustedCode(request))
    );
  }
}
