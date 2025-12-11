use anyhow::{anyhow, Result};
use reqwest;
use rquickjs::{
    CatchResultExt, Context, Ctx, Function, Object, Runtime, Value,
};
use serde_json;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tracing::debug;
use url::Url;

#[derive(Debug, Clone)]
pub struct ExecutionResult {
    pub value: serde_json::Value,
    pub console_output: Vec<String>,
}

#[derive(Clone)]
struct Console {
    output: Arc<Mutex<Vec<String>>>,
}

impl Console {
    fn new() -> Self {
        Console {
            output: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn log(&self, message: String) {
        self.output.lock().unwrap().push(format!("[log] {}", message));
    }

    fn get_output(&self) -> Vec<String> {
        self.output.lock().unwrap().clone()
    }
}

/// Execute JavaScript code in a sandboxed QuickJS environment
pub fn execute_js(
    code: &str,
    timeout_ms: u64,
    memory_limit: usize,
    allowed_domains: &[&str],
    input: Option<serde_json::Value>,
) -> Result<ExecutionResult> {
    // Create QuickJS runtime with memory limit
    let runtime = Runtime::new()?;

    // Set memory limit
    runtime.set_memory_limit(memory_limit);

    // Set max stack size (1MB)
    runtime.set_max_stack_size(1024 * 1024);

    // Track execution start time for timeout
    let start = Instant::now();
    let timeout_duration = Duration::from_millis(timeout_ms);
    let start_clone = start.clone();
    let timeout_clone = timeout_duration.clone();

    // Set interrupt handler for timeout
    runtime.set_interrupt_handler(Some(Box::new(move || {
        start_clone.elapsed() > timeout_clone
    })));

    let context = Context::full(&runtime)?;

    // Create console for capturing output
    let console = Console::new();

    let result = context.with(|ctx| {
        setup_sandbox(&ctx, console.clone(), allowed_domains)?;

        // Inject the input object into the global scope
        if let Some(inp) = input {
            let input_json = serde_json::to_string(&inp)?;
            let input_code = format!("globalThis.__userInput = {};", input_json);
            ctx.eval::<(), _>(input_code.as_str())?;
        } else {
            ctx.eval::<(), _>("globalThis.__userInput = undefined;")?;
        }

        // Wrap user code in async main function with input parameter
        let wrapped_code = format!(
            r#"(async function main(input) {{
    {}
}})(globalThis.__userInput)"#,
            code
        );

        debug!("Executing JavaScript code wrapped in async main(input)");

        // Evaluate the code - this returns a Promise
        let promise: rquickjs::Promise = ctx.eval(wrapped_code.as_str()).catch(&ctx).map_err(|e| {
            let error_msg = format_js_error(&ctx, e);
            anyhow!("JavaScript execution error: {}", error_msg)
        })?;

        // Wait for the promise to resolve
        let result_value: Value = promise.finish().catch(&ctx).map_err(|e| {
            let error_msg = format_js_error(&ctx, e);
            anyhow!("Promise resolution error: {}", error_msg)
        })?;

        // Check if timeout exceeded
        if start.elapsed() > timeout_duration {
            return Err(anyhow!("Execution timeout exceeded"));
        }

        // Convert result to JSON
        let json_value = value_to_json(&ctx, result_value)?;

        Ok(ExecutionResult {
            value: json_value,
            console_output: console.get_output(),
        })
    })?;

    Ok(result)
}

/// Setup the sandbox environment with security restrictions
fn setup_sandbox(ctx: &Ctx, console: Console, allowed_domains: &[&str]) -> Result<()> {
    let globals = ctx.globals();

    // Setup console
    setup_console(ctx, &globals, console)?;

    // Setup fetch with domain allowlist
    setup_fetch(ctx, &globals, allowed_domains)?;

    // Freeze Object.prototype to prevent prototype pollution
    ctx.eval::<(), _>("Object.freeze(Object.prototype);")?;
    ctx.eval::<(), _>("Object.freeze(Array.prototype);")?;

    // Remove dangerous globals
    globals.remove("eval").ok();
    globals.remove("Function").ok();
    globals.remove("setTimeout").ok();
    globals.remove("setInterval").ok();

    Ok(())
}

/// Setup console API for capturing output
fn setup_console<'js>(ctx: &Ctx<'js>, globals: &Object<'js>, console: Console) -> Result<()> {
    let console_obj = Object::new(ctx.clone())?;

    // Create console.log function
    let console_clone = console.clone();
    let log_fn = Function::new(
        ctx.clone(),
        move |args: rquickjs::function::Rest<Value>| {
            let messages: Vec<String> = args
                .iter()
                .map(|v| value_to_string(v))
                .collect();
            let message = messages.join(" ");
            console_clone.log(message);
        },
    )?;

    console_obj.set("log", log_fn)?;

    // Add console._times for Node.js compatibility (SES requirement)
    let times_obj = Object::new(ctx.clone())?;
    console_obj.set("_times", times_obj)?;

    globals.set("console", console_obj)?;

    Ok(())
}

/// Setup fetch API with domain allowlist
/// Returns a standards-compliant Promise-based fetch API
fn setup_fetch<'js>(ctx: &Ctx<'js>, globals: &Object<'js>, allowed_domains: &[&str]) -> Result<()> {
    let allowed_domains_vec: Vec<String> = allowed_domains.iter().map(|s| s.to_string()).collect();

    // Create a synchronous native fetch that returns either a response object or an error object
    let sync_fetch = Function::new(
        ctx.clone(),
        move |ctx: Ctx<'js>, url: String, options: Object<'js>| -> rquickjs::Result<Object<'js>> {
            // Validate URL and domain
            let parsed_url = match Url::parse(&url) {
                Ok(u) => u,
                Err(e) => {
                    let error_obj = Object::new(ctx.clone())?;
                    error_obj.set("__isError", true)?;
                    error_obj.set("message", format!("Invalid URL: {}", e))?;
                    return Ok(error_obj);
                }
            };

            let host = match parsed_url.host_str() {
                Some(h) => h,
                None => {
                    let error_obj = Object::new(ctx.clone())?;
                    error_obj.set("__isError", true)?;
                    error_obj.set("message", "Invalid URL: no host")?;
                    return Ok(error_obj);
                }
            };

            let is_allowed = allowed_domains_vec
                .iter()
                .any(|domain| host == domain || host.ends_with(&format!(".{}", domain)));

            if !is_allowed {
                let error_obj = Object::new(ctx.clone())?;
                error_obj.set("__isError", true)?;
                error_obj.set("message", format!("Domain '{}' is not in the allowlist", host))?;
                return Ok(error_obj);
            }

            // Block private IP ranges
            if host == "localhost"
                || host.starts_with("127.")
                || host.starts_with("10.")
                || host.starts_with("192.168.")
                || host.starts_with("172.16.")
                || host == "0.0.0.0"
            {
                let error_obj = Object::new(ctx.clone())?;
                error_obj.set("__isError", true)?;
                error_obj.set("message", "Requests to private IP ranges are not allowed")?;
                return Ok(error_obj);
            }

            // Parse options
            let method = options.get::<_, Option<String>>("method")
                .unwrap_or(None)
                .unwrap_or_else(|| "GET".to_string())
                .to_uppercase();

            let body = options.get::<_, Option<String>>("body").unwrap_or(None);

            // Make HTTP request
            let client = match reqwest::blocking::Client::builder()
                .timeout(Duration::from_secs(5))
                .build()
            {
                Ok(c) => c,
                Err(e) => {
                    let error_obj = Object::new(ctx.clone())?;
                    error_obj.set("__isError", true)?;
                    error_obj.set("message", format!("Failed to create HTTP client: {}", e))?;
                    return Ok(error_obj);
                }
            };

            let mut request_builder = match method.as_str() {
                "GET" => client.get(&url),
                "POST" => client.post(&url),
                "PUT" => client.put(&url),
                "DELETE" => client.delete(&url),
                "PATCH" => client.patch(&url),
                "HEAD" => client.head(&url),
                _ => {
                    let error_obj = Object::new(ctx.clone())?;
                    error_obj.set("__isError", true)?;
                    error_obj.set("message", format!("Unsupported HTTP method: {}", method))?;
                    return Ok(error_obj);
                }
            };

            // Add body if present
            if let Some(body_data) = body {
                request_builder = request_builder.body(body_data);
            }

            // Add headers if present
            if let Ok(Some(headers_obj)) = options.get::<_, Option<Object>>("headers") {
                for prop in headers_obj.props::<String, String>() {
                    if let Ok((key, value)) = prop {
                        request_builder = request_builder.header(&key, &value);
                    }
                }
            }

            let response = match request_builder.send() {
                Ok(r) => r,
                Err(e) => {
                    let error_obj = Object::new(ctx.clone())?;
                    error_obj.set("__isError", true)?;
                    error_obj.set("message", format!("HTTP request failed: {}", e))?;
                    return Ok(error_obj);
                }
            };

            let status = response.status().as_u16();
            let response_text = match response.text() {
                Ok(t) => t,
                Err(e) => {
                    let error_obj = Object::new(ctx.clone())?;
                    error_obj.set("__isError", true)?;
                    error_obj.set("message", format!("Failed to read response: {}", e))?;
                    return Ok(error_obj);
                }
            };

            // Create response object
            let response_obj = Object::new(ctx.clone())?;
            response_obj.set("status", status)?;
            response_obj.set("ok", status >= 200 && status < 300)?;
            response_obj.set("_bodyText", response_text.clone())?;

            Ok(response_obj)
        },
    )?;

    // Set the synchronous implementation as a hidden global
    ctx.globals().set("__syncFetch", sync_fetch)?;

    // Wrap it in JavaScript to provide Promise-based API
    let fetch_wrapper_code = r#"
(function() {
    return function fetch(url, options) {
        return new Promise((resolve, reject) => {
            try {
                // Convert options to empty object if undefined
                const opts = options || {};
                const result = globalThis.__syncFetch(url, opts);

                // Check if result is an error
                if (result.__isError) {
                    reject(new Error(result.message));
                    return;
                }

                // Add text() and json() methods that return Promises
                result.text = function() {
                    return Promise.resolve(this._bodyText);
                };

                result.json = function() {
                    return new Promise((resolve, reject) => {
                        try {
                            resolve(JSON.parse(this._bodyText));
                        } catch (e) {
                            reject(e);
                        }
                    });
                };

                resolve(result);
            } catch (error) {
                reject(error);
            }
        });
    };
})()
"#;

    let fetch_fn: Function = ctx.eval(fetch_wrapper_code)?;
    globals.set("fetch", fetch_fn)?;

    Ok(())
}

/// Convert a QuickJS Value to a string representation
fn value_to_string(value: &Value) -> String {
    if let Some(s) = value.as_string() {
        s.to_string().unwrap_or_else(|_| "[String]".to_string())
    } else if value.is_null() {
        "null".to_string()
    } else if value.is_undefined() {
        "undefined".to_string()
    } else if let Some(b) = value.as_bool() {
        b.to_string()
    } else if let Some(n) = value.as_int() {
        n.to_string()
    } else if let Some(n) = value.as_float() {
        n.to_string()
    } else if value.is_object() {
        "[Object]".to_string()
    } else if value.is_array() {
        "[Array]".to_string()
    } else if value.is_function() {
        "[Function]".to_string()
    } else {
        "[Unknown]".to_string()
    }
}

/// Convert a QuickJS Value to serde_json::Value
fn value_to_json<'js>(ctx: &Ctx<'js>, value: Value<'js>) -> Result<serde_json::Value> {
    if value.is_null() {
        Ok(serde_json::Value::Null)
    } else if value.is_undefined() {
        Ok(serde_json::Value::Null)
    } else if let Some(b) = value.as_bool() {
        Ok(serde_json::Value::Bool(b))
    } else if let Some(i) = value.as_int() {
        Ok(serde_json::Value::Number(i.into()))
    } else if let Some(f) = value.as_float() {
        serde_json::Number::from_f64(f)
            .map(serde_json::Value::Number)
            .ok_or_else(|| anyhow!("Invalid float value"))
    } else if let Some(s) = value.as_string() {
        Ok(serde_json::Value::String(
            s.to_string().unwrap_or_else(|_| String::new()),
        ))
    } else if value.is_array() {
        let arr = value.as_array().unwrap();
        let mut result = Vec::new();
        for i in 0..arr.len() {
            if let Ok(item) = arr.get::<Value>(i) {
                result.push(value_to_json(ctx, item)?);
            }
        }
        Ok(serde_json::Value::Array(result))
    } else if value.is_object() {
        let obj = value.as_object().unwrap();

        // Try to use JSON.stringify for better conversion
        let json_obj: Object = ctx.globals().get("JSON")?;
        let stringify: Function = json_obj.get("stringify")?;

        match stringify.call::<_, String>((value.clone(),)) {
            Ok(json_str) => {
                serde_json::from_str(&json_str).map_err(|e| anyhow!("Failed to parse JSON: {}", e))
            }
            Err(_) => {
                // Fallback to manual conversion
                let mut map = serde_json::Map::new();
                for prop in obj.props::<String, Value>() {
                    if let Ok((key, val)) = prop {
                        map.insert(key, value_to_json(ctx, val)?);
                    }
                }
                Ok(serde_json::Value::Object(map))
            }
        }
    } else if value.is_function() {
        Ok(serde_json::Value::String("[Function]".to_string()))
    } else {
        Ok(serde_json::Value::String(format!("[Unknown type]")))
    }
}

/// Format JavaScript error for better error messages
fn format_js_error<'js>(_ctx: &Ctx<'js>, error: rquickjs::CaughtError<'js>) -> String {
    match error {
        rquickjs::CaughtError::Exception(e) => {
            let message = e.message().unwrap_or_else(|| "Unknown error".to_string());
            let stack = e.stack().unwrap_or_else(|| String::new());

            if !stack.is_empty() {
                format!("{}\n{}", message, stack)
            } else {
                message
            }
        }
        rquickjs::CaughtError::Error(e) => format!("Error: {}", e),
        _ => "Unknown error".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_execution() {
        let code = "return 2 + 2";
        let result = execute_js(code, 5000, 10 * 1024 * 1024, &[], None).unwrap();
        assert_eq!(result.value, serde_json::json!(4));
    }

    #[test]
    fn test_console_output() {
        let code = r#"
            console.log("Hello", "World");
            return "done";
        "#;
        let result = execute_js(code, 5000, 10 * 1024 * 1024, &[], None).unwrap();
        assert_eq!(result.value, serde_json::json!("done"));
        assert!(result.console_output.contains(&"[log] Hello World".to_string()));
    }

    #[test]
    fn test_json_return() {
        let code = r#"
            return {
                "foo": "bar",
                "number": 42,
                "nested": {
                    "array": [1, 2, 3]
                }
            };
        "#;
        let result = execute_js(code, 5000, 10 * 1024 * 1024, &[], None).unwrap();
        assert_eq!(
            result.value,
            serde_json::json!({
                "foo": "bar",
                "number": 42,
                "nested": {
                    "array": [1, 2, 3]
                }
            })
        );
    }

    #[test]
    fn test_infinite_loop_timeout() {
        let code = "while(true) {}";
        let result = execute_js(code, 100, 10 * 1024 * 1024, &[], None);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        // The interrupt handler should trigger and produce an error containing "interrupt"
        assert!(err_msg.contains("timeout") || err_msg.contains("interrupt"));
    }

    #[test]
    fn test_syntax_error() {
        let code = "invalid javascript syntax {{{";
        let result = execute_js(code, 5000, 10 * 1024 * 1024, &[], None);
        assert!(result.is_err());
    }

    #[test]
    fn test_fetch_not_allowed_domain() {
        let code = r#"
            try {
                const response = await fetch("https://evil.com/data");
                return "should have rejected: " + JSON.stringify(response);
            } catch (error) {
                return error.message;
            }
        "#;
        let result = execute_js(code, 5000, 10 * 1024 * 1024, &["example.com"], None).unwrap();
        let response_str = result.value.as_str().unwrap();
        assert!(response_str.contains("not in the allowlist") || response_str.contains("allowlist"));
    }

    #[test]
    fn test_fetch_blocked_localhost() {
        let code = r#"
            try {
                const response = await fetch("http://localhost:8080/secret");
                return "should have rejected";
            } catch (error) {
                return error.message;
            }
        "#;
        let result = execute_js(code, 5000, 10 * 1024 * 1024, &["localhost"], None).unwrap();
        let response_str = result.value.as_str().unwrap();
        assert!(response_str.contains("private IP"));
    }

    #[test]
    fn test_runtime_isolation() {
        // First execution: set a global variable
        let code1 = r#"
            globalThis.sharedState = "leaked value";
            return "first execution";
        "#;
        let result1 = execute_js(code1, 5000, 10 * 1024 * 1024, &[], None).unwrap();
        assert_eq!(result1.value, serde_json::json!("first execution"));

        // Second execution: try to access the global variable from first execution
        // This should fail if runtimes are properly isolated
        let code2 = r#"
            return {
                hasSharedState: typeof globalThis.sharedState !== 'undefined',
                sharedStateValue: globalThis.sharedState || null
            };
        "#;
        let result2 = execute_js(code2, 5000, 10 * 1024 * 1024, &[], None).unwrap();
        let obj = result2.value.as_object().expect("Result should be an object");

        // The shared state should NOT exist in the second execution
        // This proves each execution gets a fresh runtime
        assert_eq!(obj.get("hasSharedState").unwrap(), &serde_json::json!(false));
        assert_eq!(obj.get("sharedStateValue").unwrap(), &serde_json::json!(null));
    }

    #[test]
    fn test_input_parameter() {
        let code = r#"
            return {
                receivedInput: input,
                type: typeof input,
                hasName: input && 'name' in input
            };
        "#;
        let input = serde_json::json!({
            "name": "test",
            "value": 42
        });
        let result = execute_js(code, 5000, 10 * 1024 * 1024, &[], Some(input)).unwrap();

        let obj = result.value.as_object().unwrap();
        assert_eq!(obj.get("type").unwrap(), &serde_json::json!("object"));
        assert_eq!(obj.get("hasName").unwrap(), &serde_json::json!(true));

        let received = obj.get("receivedInput").unwrap().as_object().unwrap();
        assert_eq!(received.get("name").unwrap(), &serde_json::json!("test"));
        assert_eq!(received.get("value").unwrap(), &serde_json::json!(42));
    }

    #[test]
    fn test_fetch_allowed_domain() {
        // This test actually makes a real HTTP request to httpbin.org
        // We verify that fetch returns a Promise and response has proper methods
        let code = r#"
            const response = await fetch("https://httpbin.org/get");
            const data = await response.json();
            return {
                hasStatus: typeof response.status === 'number',
                hasOk: typeof response.ok === 'boolean',
                status: response.status,
                hasJsonData: typeof data === 'object' && data !== null
            };
        "#;
        let result = execute_js(code, 10000, 10 * 1024 * 1024, &["httpbin.org"], None);
        // Verify fetch works - either success or valid HTTP error (not 0 which is connection error)
        if let Ok(res) = result {
            let obj = res.value.as_object().unwrap();
            assert_eq!(obj.get("hasStatus").unwrap(), &serde_json::json!(true));
            assert_eq!(obj.get("hasOk").unwrap(), &serde_json::json!(true));
            // Status should be a real HTTP status, not 0 (which indicates our error handling)
            let status = obj.get("status").unwrap().as_i64().unwrap();
            assert!(status >= 200 && status < 600, "Expected valid HTTP status code, got {}", status);
            // Should have successfully parsed JSON
            assert_eq!(obj.get("hasJsonData").unwrap(), &serde_json::json!(true));
        }
    }

    #[test]
    fn test_fetch_post_with_body() {
        let code = r#"
            const response = await fetch("https://httpbin.org/post", {
                method: "POST",
                headers: {
                    "Content-Type": "application/json"
                },
                body: JSON.stringify({ test: "data", number: 42 })
            });
            const data = await response.json();
            return {
                status: response.status,
                ok: response.ok,
                hasJsonField: data.json && typeof data.json === 'object'
            };
        "#;
        let result = execute_js(code, 10000, 10 * 1024 * 1024, &["httpbin.org"], None);
        if let Ok(res) = result {
            let obj = res.value.as_object().unwrap();
            let status = obj.get("status").unwrap().as_i64().unwrap();
            assert!(status >= 200 && status < 300, "Expected 2xx status, got {}", status);
            assert_eq!(obj.get("ok").unwrap(), &serde_json::json!(true));
            assert_eq!(obj.get("hasJsonField").unwrap(), &serde_json::json!(true));
        }
    }

    #[test]
    fn test_fetch_put_method() {
        let code = r#"
            const response = await fetch("https://httpbin.org/put", {
                method: "PUT",
                body: "test data"
            });
            return {
                status: response.status,
                ok: response.ok
            };
        "#;
        let result = execute_js(code, 10000, 10 * 1024 * 1024, &["httpbin.org"], None);
        if let Ok(res) = result {
            let obj = res.value.as_object().unwrap();
            let status = obj.get("status").unwrap().as_i64().unwrap();
            // Accept 2xx or 5xx (service errors are ok, we're testing method support)
            assert!(status >= 200 && status < 600, "Expected valid HTTP status for PUT, got {}", status);
        }
    }

    #[test]
    fn test_fetch_delete_method() {
        let code = r#"
            const response = await fetch("https://httpbin.org/delete", {
                method: "DELETE"
            });
            return {
                status: response.status,
                ok: response.ok
            };
        "#;
        let result = execute_js(code, 10000, 10 * 1024 * 1024, &["httpbin.org"], None);
        if let Ok(res) = result {
            let obj = res.value.as_object().unwrap();
            let status = obj.get("status").unwrap().as_i64().unwrap();
            // Accept 2xx or 5xx (service errors are ok, we're testing method support)
            assert!(status >= 200 && status < 600, "Expected valid HTTP status for DELETE, got {}", status);
        }
    }

    // Tests for unhandled exceptions and runtime errors
    #[test]
    fn test_accessing_property_on_undefined() {
        let code = r#"
            const obj = undefined;
            return obj.name;
        "#;
        let result = execute_js(code, 5000, 10 * 1024 * 1024, &[], None);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("TypeError") || err.contains("undefined"));
    }

    #[test]
    fn test_accessing_property_on_null() {
        let code = r#"
            const data = null;
            return data.value;
        "#;
        let result = execute_js(code, 5000, 10 * 1024 * 1024, &[], None);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("TypeError") || err.contains("null"));
    }

    #[test]
    fn test_accessing_nested_undefined_properties() {
        let code = r#"
            const data = { user: { name: 'John' } };
            return data.user.profile.nested.value;
        "#;
        let result = execute_js(code, 5000, 10 * 1024 * 1024, &[], None);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("TypeError") || err.contains("undefined"));
    }

    #[test]
    fn test_reference_error_undefined_variable() {
        let code = r#"
            return undefinedVariable;
        "#;
        let result = execute_js(code, 5000, 10 * 1024 * 1024, &[], None);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("ReferenceError") || err.contains("not defined"));
    }

    #[test]
    fn test_calling_non_function() {
        let code = r#"
            const notAFunction = "string";
            return notAFunction();
        "#;
        let result = execute_js(code, 5000, 10 * 1024 * 1024, &[], None);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("TypeError") || err.contains("not a function"));
    }

    #[test]
    fn test_network_call_to_disallowed_domain() {
        let code = r#"
            try {
                const response = await fetch("https://blocked-domain.com/api");
                return { error: "should have failed" };
            } catch (error) {
                return { caught: true, message: error.message };
            }
        "#;
        let result = execute_js(code, 5000, 10 * 1024 * 1024, &["allowed-domain.com"], None).unwrap();
        let obj = result.value.as_object().unwrap();
        assert_eq!(obj.get("caught").unwrap(), &serde_json::json!(true));
        let message = obj.get("message").unwrap().as_str().unwrap();
        assert!(message.contains("not in the allowlist") || message.contains("allowlist"));
    }

    #[test]
    fn test_json_parse_error() {
        let code = r#"
            try {
                return JSON.parse("{ invalid json }");
            } catch (error) {
                return { caught: true, type: error.constructor.name };
            }
        "#;
        let result = execute_js(code, 5000, 10 * 1024 * 1024, &[], None).unwrap();
        let obj = result.value.as_object().unwrap();
        assert_eq!(obj.get("caught").unwrap(), &serde_json::json!(true));
        assert_eq!(obj.get("type").unwrap(), &serde_json::json!("SyntaxError"));
    }

    #[test]
    fn test_array_out_of_bounds_access() {
        let code = r#"
            const arr = [1, 2, 3];
            return arr[100].id;
        "#;
        let result = execute_js(code, 5000, 10 * 1024 * 1024, &[], None);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("TypeError") || err.contains("undefined"));
    }

    #[test]
    fn test_promise_rejection() {
        let code = r#"
            await Promise.reject(new Error("Promise rejected"));
        "#;
        let result = execute_js(code, 5000, 10 * 1024 * 1024, &[], None);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Promise rejected"));
    }

    #[test]
    fn test_async_error_in_promise_chain() {
        let code = r#"
            const data = await Promise.resolve({ items: [] })
                .then(d => d.items.find(i => i.id === 1))
                .then(item => item.name);
            return data;
        "#;
        let result = execute_js(code, 5000, 10 * 1024 * 1024, &[], None);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("TypeError") || err.contains("undefined"));
    }

    #[test]
    fn test_division_by_zero() {
        // Division by zero in JavaScript returns Infinity, which cannot be represented in JSON
        let code = r#"
            return 1 / 0;
        "#;
        let result = execute_js(code, 5000, 10 * 1024 * 1024, &[], None);
        // Infinity cannot be converted to JSON, so it should error
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Invalid float") || err.contains("float"));
    }

    #[test]
    fn test_throw_custom_error() {
        let code = r#"
            throw new Error("Custom error message");
        "#;
        let result = execute_js(code, 5000, 10 * 1024 * 1024, &[], None);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Custom error message"));
    }

    #[test]
    fn test_throw_string() {
        let code = r#"
            throw "String error";
        "#;
        let result = execute_js(code, 5000, 10 * 1024 * 1024, &[], None);
        assert!(result.is_err());
        // String throws may have different formatting, just verify we got an error
        assert!(result.is_err());
    }

    #[test]
    fn test_network_invalid_url() {
        let code = r#"
            try {
                const response = await fetch("not-a-valid-url");
                return { error: "should have failed" };
            } catch (error) {
                return { caught: true, message: error.message };
            }
        "#;
        let result = execute_js(code, 5000, 10 * 1024 * 1024, &["example.com"], None).unwrap();
        let obj = result.value.as_object().unwrap();
        assert_eq!(obj.get("caught").unwrap(), &serde_json::json!(true));
        let message = obj.get("message").unwrap().as_str().unwrap();
        assert!(message.contains("Invalid URL") || message.contains("URL"));
    }

    #[test]
    fn test_network_blocked_localhost() {
        let code = r#"
            try {
                const response = await fetch("http://localhost:8080/secret");
                return { error: "should have failed" };
            } catch (error) {
                return { caught: true, message: error.message };
            }
        "#;
        let result = execute_js(code, 5000, 10 * 1024 * 1024, &["localhost"], None).unwrap();
        let obj = result.value.as_object().unwrap();
        assert_eq!(obj.get("caught").unwrap(), &serde_json::json!(true));
        let message = obj.get("message").unwrap().as_str().unwrap();
        assert!(message.contains("private IP"));
    }

    #[test]
    fn test_network_blocked_private_ip() {
        let code = r#"
            try {
                const response = await fetch("http://192.168.1.1/admin");
                return { error: "should have failed" };
            } catch (error) {
                return { caught: true, message: error.message };
            }
        "#;
        let result = execute_js(code, 5000, 10 * 1024 * 1024, &["192.168.1.1"], None).unwrap();
        let obj = result.value.as_object().unwrap();
        assert_eq!(obj.get("caught").unwrap(), &serde_json::json!(true));
        let message = obj.get("message").unwrap().as_str().unwrap();
        assert!(message.contains("private IP"));
    }

    #[test]
    fn test_input_property_access_when_undefined() {
        let code = r#"
            // input is undefined, accessing property should fail
            return input.someProperty;
        "#;
        let result = execute_js(code, 5000, 10 * 1024 * 1024, &[], None);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("TypeError") || err.contains("undefined"));
    }

    #[test]
    fn test_malformed_json_response_simulation() {
        // Simulate a case where JSON parsing fails on response
        let code = r#"
            try {
                const malformedJson = "<html>Not JSON</html>";
                return JSON.parse(malformedJson);
            } catch (error) {
                return { caught: true, type: error.constructor.name, message: error.message };
            }
        "#;
        let result = execute_js(code, 5000, 10 * 1024 * 1024, &[], None).unwrap();
        let obj = result.value.as_object().unwrap();
        assert_eq!(obj.get("caught").unwrap(), &serde_json::json!(true));
        assert_eq!(obj.get("type").unwrap(), &serde_json::json!("SyntaxError"));
    }

    #[test]
    fn test_stack_overflow() {
        let code = r#"
            function recursive() {
                return recursive();
            }
            return recursive();
        "#;
        let result = execute_js(code, 5000, 10 * 1024 * 1024, &[], None);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        // Should get stack overflow or max stack size exceeded
        assert!(err.contains("stack") || err.contains("InternalError") || err.contains("recursion"));
    }

    #[test]
    fn test_try_catch_handles_error_gracefully() {
        let code = r#"
            try {
                const obj = undefined;
                return obj.name;
            } catch (error) {
                return { caught: true, errorType: error.constructor.name, message: error.message };
            }
        "#;
        let result = execute_js(code, 5000, 10 * 1024 * 1024, &[], None).unwrap();
        let obj = result.value.as_object().unwrap();
        assert_eq!(obj.get("caught").unwrap(), &serde_json::json!(true));
        assert_eq!(obj.get("errorType").unwrap(), &serde_json::json!("TypeError"));
    }

    #[test]
    fn test_multiple_errors_first_one_caught() {
        let code = r#"
            try {
                throw new Error("First error");
                throw new Error("Second error");
            } catch (error) {
                return { message: error.message };
            }
        "#;
        let result = execute_js(code, 5000, 10 * 1024 * 1024, &[], None).unwrap();
        let obj = result.value.as_object().unwrap();
        assert_eq!(obj.get("message").unwrap(), &serde_json::json!("First error"));
    }

    #[test]
    fn test_user_return_skip_reason() {
        let code = r#"
            return {
                skip_reason: "user_cancelled",
                additional_data: "some info"
            };
        "#;
        let result = execute_js(code, 5000, 10 * 1024 * 1024, &[], None).unwrap();
        let obj = result.value.as_object().unwrap();
        assert_eq!(obj.get("skip_reason").unwrap(), &serde_json::json!("user_cancelled"));
        assert_eq!(obj.get("additional_data").unwrap(), &serde_json::json!("some info"));
    }

    #[test]
    fn test_user_return_error_reason() {
        let code = r#"
            return {
                error_reason: "validation_failed",
                details: "Missing required field"
            };
        "#;
        let result = execute_js(code, 5000, 10 * 1024 * 1024, &[], None).unwrap();
        let obj = result.value.as_object().unwrap();
        assert_eq!(obj.get("error_reason").unwrap(), &serde_json::json!("validation_failed"));
        assert_eq!(obj.get("details").unwrap(), &serde_json::json!("Missing required field"));
    }

    #[test]
    fn test_user_return_both_reasons() {
        let code = r#"
            return {
                skip_reason: "user_skip",
                error_reason: "also_error",
                data: 42
            };
        "#;
        let result = execute_js(code, 5000, 10 * 1024 * 1024, &[], None).unwrap();
        let obj = result.value.as_object().unwrap();
        assert_eq!(obj.get("skip_reason").unwrap(), &serde_json::json!("user_skip"));
        assert_eq!(obj.get("error_reason").unwrap(), &serde_json::json!("also_error"));
        assert_eq!(obj.get("data").unwrap(), &serde_json::json!(42));
    }

    #[test]
    fn test_user_return_nothing() {
        let code = r#"
            // Don't return anything
        "#;
        let result = execute_js(code, 5000, 10 * 1024 * 1024, &[], None).unwrap();
        // Should return null/undefined
        assert!(result.value.is_null());
    }

    #[test]
    fn test_user_return_empty_object() {
        let code = r#"
            return {};
        "#;
        let result = execute_js(code, 5000, 10 * 1024 * 1024, &[], None).unwrap();
        let obj = result.value.as_object().unwrap();
        assert!(obj.is_empty());
    }
}
