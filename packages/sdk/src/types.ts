/**
 * Options for executing JavaScript code
 */
export interface RunUntrustedCodeOptions {
  /** The JavaScript code to execute */
  code: string;

  /**
   * AWS Lambda function name or ARN
   * Defaults to SANDBOX_FUNCTION_NAME environment variable
   */
  functionName?: string;

  /**
   * AWS region
   * Automatically picked up from AWS SDK default credential chain
   */
  region?: string;

  /** Optional timeout in milliseconds (default: 5000, max: 25000) */
  timeoutMs?: number;

  /** Optional memory limit in bytes (default: 10MB, max: 50MB) */
  memoryLimitBytes?: number;

  /**
   * Optional list of allowed domains for fetch API
   *
   * When provided, the sandbox will allow fetch() requests to these domains.
   * Subdomain matching is supported (e.g., "example.com" allows "api.example.com").
   * Private IP ranges (localhost, 127.x.x.x, 10.x.x.x, 192.168.x.x) are always blocked.
   *
   * @default [] (no network access)
   * @example ['api.example.com', 'httpbin.org']
   */
  allowedDomains?: string[];
}

/**
 * Request interface sent to Lambda (internal)
 */
export interface ExecuteRequest {
  code: string;
  timeoutMs?: number;
  memoryLimitBytes?: number;
  allowedDomains?: string[];
}

/**
 * Response interface from JavaScript execution
 */
export interface ExecuteResponse {
  /** Whether execution was successful */
  success: boolean;

  /** The result of the execution (if successful) */
  result?: any;

  /** Error message (if failed) */
  error?: string;

  /** Execution time in milliseconds */
  executionTimeMs: number;

  /** Console output captured during execution */
  consoleOutput: string[];
}

