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
 * Input data passed to user code
 */
export interface CodeInput {
  /** Organization ID */
  org_id?: string;

  /** Entity data */
  entity?: Record<string, unknown>;

  /** Action configuration */
  action_config?: any;

  /** Application configuration */
  app_config?: any;

  /** Application options with token */
  app_options?: Record<string, unknown>;

  /** Execution ID */
  execution_id?: string;

  /** Execution status */
  execution_status?: string;

  /** Execution action ID */
  execution_action_id?: string;

  /** Trigger event data */
  trigger_event?: any;
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
   * Optional input object to pass to the user code
   *
   * This object will be available as the `input` parameter in the async main function.
   * User code is automatically wrapped in: `(async function main(input) { <user-code> })(input)`
   *
   * To skip automation, return an object with `skip_reason` or `error_reason`:
   * `return { skip_reason: "user_cancelled", ...otherData }`
   * `return { error_reason: "validation_failed", ...otherData }`
   *
   * @default undefined
   * @example { org_id: '123', entity: {...}, action_config: {...}, app_config: {...}, app_options: {...}, execution_id: 'exec_123', execution_status: 'running', execution_action_id: 'action_456', trigger_event: {...} }
   */
  input?: CodeInput;
}

/**
 * Request interface sent to Lambda (internal)
 */
export interface ExecuteRequest {
  code: string;
  timeoutMs?: number;
  memoryLimitBytes?: number;
  allowedDomains?: string[];
  input?: CodeInput;
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

