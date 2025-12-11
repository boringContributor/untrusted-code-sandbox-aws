/**
 * Structured information extracted from user code return value
 */
export interface StructuredError {
  /**
   * Error output from unhandled errors or user-provided error_reason field.
   * Set automatically when unexpected errors occur, or by user code when
   * returning { error_reason: "reason" }.
   */
  errorOutput?: string;

  /**
   * Reason why execution was skipped.
   * Set by user code when returning { skip_reason: "reason" } to skip automation.
   */
  skipReason?: string;
}

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

  /**
   * Optional options object to pass to the user code
   *
   * This object will be available as the `options` parameter in the async main function.
   * User code is automatically wrapped in: `(async function main(options) { <user-code> })(options)`
   *
   * To skip automation, return an object with `skip_reason` or `error_reason`:
   * `return { skip_reason: "user_cancelled", ...otherData }`
   * `return { error_reason: "validation_failed", ...otherData }`
   *
   * @default undefined
   * @example { userId: '123', apiKey: 'secret' }
   */
  options?: any;
}

/**
 * Request interface sent to Lambda (internal)
 */
export interface ExecuteRequest {
  code: string;
  timeoutMs?: number;
  memoryLimitBytes?: number;
  allowedDomains?: string[];
  options?: any;
}

/**
 * Response interface from JavaScript execution
 */
export interface ExecuteResponse {
  /** Whether the execution completed successfully */
  success: boolean;

  /** The result value returned from user code (if successful) */
  result?: any;

  /** Error message from the sandbox (if execution failed) */
  error?: string;

  /** Reason why execution was skipped (from user code) */
  skip_reason?: string;

  /** Error reason from user code or unexpected errors */
  error_reason?: string;

  /** Execution time in milliseconds */
  executionTimeMs: number;

  /** Console output captured during execution */
  consoleOutput: string[];
}

