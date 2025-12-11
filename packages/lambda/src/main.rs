mod sandbox;

use lambda_runtime::{run, service_fn, Error, LambdaEvent};
use serde::{Deserialize, Serialize};
use tracing::info;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExecuteRequest {
    /// The JavaScript code to execute
    code: String,

    /// Optional timeout in milliseconds (default: 5000, max: 25000)
    #[serde(default = "default_timeout")]
    timeout_ms: u64,

    /// Optional memory limit in bytes (default: 10MB)
    #[serde(default = "default_memory_limit")]
    memory_limit_bytes: usize,

    /// List of allowed domains for fetch API (default: empty)
    #[serde(default)]
    allowed_domains: Vec<String>,

    /// Optional options object to pass to the main function
    #[serde(default)]
    options: Option<serde_json::Value>,
}

fn default_timeout() -> u64 {
    5000 // 5 seconds
}

fn default_memory_limit() -> usize {
    10 * 1024 * 1024 // 10 MB
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ExecuteResponse {
    /// Whether execution was successful
    success: bool,

    /// The result of the execution (if successful)
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<serde_json::Value>,

    /// Error message (if failed)
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,

    /// Reason why execution was skipped (from user code)
    #[serde(skip_serializing_if = "Option::is_none")]
    skip_reason: Option<String>,

    /// Error reason (from user code or unexpected errors)
    #[serde(skip_serializing_if = "Option::is_none")]
    error_reason: Option<String>,

    /// Execution time in milliseconds
    execution_time_ms: u128,

    /// Console output captured during execution
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    console_output: Vec<String>,
}

async fn function_handler(event: LambdaEvent<ExecuteRequest>) -> Result<ExecuteResponse, Error> {
    let (request, _context) = event.into_parts();

    info!("Executing JavaScript code (length: {} bytes)", request.code.len());

    // Validate input
    if request.code.is_empty() {
        return Ok(ExecuteResponse {
            success: false,
            result: None,
            error: Some("Code cannot be empty".to_string()),
            skip_reason: None,
            error_reason: None,
            execution_time_ms: 0,
            console_output: Vec::new(),
        });
    }

    // Limit code size to prevent abuse
    const MAX_CODE_SIZE: usize = 100 * 1024; // 100 KB
    if request.code.len() > MAX_CODE_SIZE {
        return Ok(ExecuteResponse {
            success: false,
            result: None,
            error: Some(format!("Code size exceeds maximum of {} bytes", MAX_CODE_SIZE)),
            skip_reason: None,
            error_reason: None,
            execution_time_ms: 0,
            console_output: Vec::new(),
        });
    }

    // Validate timeout
    const MAX_TIMEOUT_MS: u64 = 25000; // 25 seconds (within Lambda timeout)
    let timeout_ms = request.timeout_ms.min(MAX_TIMEOUT_MS);

    // Validate memory limit
    const MAX_MEMORY_LIMIT: usize = 50 * 1024 * 1024; // 50 MB
    let memory_limit = request.memory_limit_bytes.min(MAX_MEMORY_LIMIT);

    let start = std::time::Instant::now();

    // Convert allowed_domains to &[&str]
    let allowed_domains_refs: Vec<&str> = request.allowed_domains.iter().map(|s| s.as_str()).collect();

    // Execute the code in sandbox
    match sandbox::execute_js(
        &request.code,
        timeout_ms,
        memory_limit,
        &allowed_domains_refs,
        request.options,
    ) {
        Ok(result) => {
            let execution_time = start.elapsed().as_millis();

            // Extract skip_reason and error_reason from the result if present
            let mut skip_reason = None;
            let mut error_reason = None;

            if let Some(obj) = result.value.as_object() {
                if let Some(reason) = obj.get("skip_reason") {
                    if let Some(reason_str) = reason.as_str() {
                        skip_reason = Some(reason_str.to_string());
                        info!("Execution completed with skip_reason: {} (took {}ms)", reason_str, execution_time);
                    }
                }
                if let Some(reason) = obj.get("error_reason") {
                    if let Some(reason_str) = reason.as_str() {
                        error_reason = Some(reason_str.to_string());
                        info!("Execution completed with error_reason: {} (took {}ms)", reason_str, execution_time);
                    }
                }
            }

            if skip_reason.is_none() && error_reason.is_none() {
                info!("Execution successful (took {}ms)", execution_time);
            }

            Ok(ExecuteResponse {
                success: true,
                result: Some(result.value),
                error: None,
                skip_reason,
                error_reason,
                execution_time_ms: execution_time,
                console_output: result.console_output,
            })
        }
        Err(e) => {
            let execution_time = start.elapsed().as_millis();
            let error_msg = e.to_string();
            info!("Execution failed: {} (took {}ms)", error_msg, execution_time);

            Ok(ExecuteResponse {
                success: false,
                result: None,
                error: Some(error_msg.clone()),
                skip_reason: None,
                error_reason: Some(error_msg), // Forward unexpected errors to error_reason
                execution_time_ms: execution_time,
                console_output: Vec::new(),
            })
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_target(false)
        .without_time()
        .init();

    info!("Starting JavaScript executor Lambda function");

    run(service_fn(function_handler)).await
}
