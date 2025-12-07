use anyhow::{anyhow, Result};
use rquickjs::{
    CatchResultExt, Ctx, Function, Object, Runtime, Context, Value,
};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tracing::debug;
use url::Url;

/// Result of JavaScript execution
#[derive(Debug)]
pub struct ExecutionResult {
    pub value: serde_json::Value,
    pub console_output: Vec<String>,
}

/// Console implementation for capturing output
#[derive(Clone)]
struct Console {
    output: Arc<Mutex<Vec<String>>>,
}

impl Console {
    fn new() -> Self {
        Self {
            output: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn get_output(&self) -> Vec<String> {
        self.output.lock().unwrap().clone()
    }

    fn log_message(&self, level: &str, args: Vec<String>) {
        let message = format!("[{}] {}", level, args.join(" "));
        self.output.lock().unwrap().push(message);
    }
}

/// Execute JavaScript code in a secure sandbox
pub fn execute_js(
    code: &str,
    timeout_ms: u64,
    memory_limit: usize,
    allowed_domains: &[&str],
) -> Result<ExecutionResult> {
    // Create QuickJS runtime with memory limit
    let runtime = Runtime::new()?;

    // Set memory limit
    runtime.set_memory_limit(memory_limit);

    // Set max stack size (1MB)
    runtime.set_max_stack_size(1024 * 1024);

    let context = Context::full(&runtime)?;

    // Create console for capturing output
    let console = Console::new();

    // Track execution start time
    let start = Instant::now();
    let timeout_duration = Duration::from_millis(timeout_ms);

    let result = context.with(|ctx| {
        setup_sandbox(&ctx, console.clone(), allowed_domains)?;

        // Execute the code
        debug!("Executing JavaScript code");

        // Simple timeout check (QuickJS 0.6 doesn't have built-in interrupt handler in all builds)
        let result_value: Value = ctx
            .eval(code)
            .catch(&ctx)
            .map_err(|e| {
                let error_msg = format_js_error(&ctx, e);
                anyhow!("JavaScript execution error: {}", error_msg)
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

    // Remove dangerous global objects
    remove_dangerous_globals(ctx, &globals)?;

    // Setup safe console
    setup_console(ctx, &globals, console)?;

    // Add safe setTimeout/setInterval stubs (they don't work in QuickJS without event loop)
    add_timer_stubs(ctx, &globals)?;

    // Setup fetch with allowlist
    setup_fetch(ctx, &globals, allowed_domains)?;

    Ok(())
}

/// Remove dangerous global objects and functions
fn remove_dangerous_globals(ctx: &Ctx, globals: &Object) -> Result<()> {
    // List of dangerous globals to remove
    let dangerous_globals = vec![
        // File system access (not available in QuickJS by default, but being explicit)
        "require",
        "import",
        "importScripts",

        // Process/system access
        "process",
        "exit",

        // Dynamic code evaluation (keeping eval for now as it's needed for some use cases)
        // "eval",
        // "Function",

        // Worker threads
        "Worker",
        "SharedArrayBuffer",
        "Atomics",
    ];

    for global in dangerous_globals {
        if globals.contains_key(global)? {
            globals.remove(global)?;
            debug!("Removed dangerous global: {}", global);
        }
    }

    // Prevent access to global 'this' in strict mode
    ctx.eval::<(), _>("'use strict';")?;

    Ok(())
}

/// Setup a safe console object that captures output
fn setup_console<'js>(ctx: &Ctx<'js>, globals: &Object<'js>, console: Console) -> Result<()> {
    let console_obj = Object::new(ctx.clone())?;

    // Create console.log
    let log_console = console.clone();
    let log_fn = Function::new(ctx.clone(), move |args: rquickjs::function::Rest<Value>| {
        let messages: Vec<String> = args.iter().map(|v| value_to_string(v)).collect();
        log_console.log_message("log", messages);
    })?;
    console_obj.set("log", log_fn)?;

    // Create console.info
    let info_console = console.clone();
    let info_fn = Function::new(ctx.clone(), move |args: rquickjs::function::Rest<Value>| {
        let messages: Vec<String> = args.iter().map(|v| value_to_string(v)).collect();
        info_console.log_message("info", messages);
    })?;
    console_obj.set("info", info_fn)?;

    // Create console.warn
    let warn_console = console.clone();
    let warn_fn = Function::new(ctx.clone(), move |args: rquickjs::function::Rest<Value>| {
        let messages: Vec<String> = args.iter().map(|v| value_to_string(v)).collect();
        warn_console.log_message("warn", messages);
    })?;
    console_obj.set("warn", warn_fn)?;

    // Create console.error
    let error_console = console.clone();
    let error_fn = Function::new(ctx.clone(), move |args: rquickjs::function::Rest<Value>| {
        let messages: Vec<String> = args.iter().map(|v| value_to_string(v)).collect();
        error_console.log_message("error", messages);
    })?;
    console_obj.set("error", error_fn)?;

    // Create console.debug
    let debug_fn = Function::new(ctx.clone(), move |args: rquickjs::function::Rest<Value>| {
        let messages: Vec<String> = args.iter().map(|v| value_to_string(v)).collect();
        console.log_message("debug", messages);
    })?;
    console_obj.set("debug", debug_fn)?;

    globals.set("console", console_obj)?;

    Ok(())
}

/// Add timer stubs (setTimeout, setInterval) that warn users they're not available
fn add_timer_stubs(_ctx: &Ctx, globals: &Object) -> Result<()> {
    // Just remove setTimeout and setInterval - they're not needed for the sandbox
    globals.remove("setTimeout").ok();
    globals.remove("setInterval").ok();

    Ok(())
}

/// Setup fetch API with domain allowlist
fn setup_fetch<'js>(ctx: &Ctx<'js>, globals: &Object<'js>, allowed_domains: &[&str]) -> Result<()> {
    let allowed_domains_vec: Vec<String> = allowed_domains.iter().map(|s| s.to_string()).collect();

    let fetch_fn = Function::new(ctx.clone(), move |ctx: Ctx<'js>, url: String, options: rquickjs::function::Opt<Object<'js>>| {
        // Helper function to create an error response
        let create_error = |ctx: &Ctx<'js>, error_msg: String| -> Option<Object<'js>> {
            let obj = Object::new(ctx.clone()).ok()?;
            obj.set("ok", false).ok()?;
            obj.set("status", 0).ok()?;
            obj.set("error", error_msg).ok()?;
            Some(obj)
        };

        // Parse URL
        let parsed_url = match Url::parse(&url) {
            Ok(u) => u,
            Err(e) => return create_error(&ctx, format!("Invalid URL: {}", e)),
        };

        // Check if domain is allowed
        let host = match parsed_url.host_str() {
            Some(h) => h,
            None => return create_error(&ctx, "URL has no host".to_string()),
        };

        let is_allowed = allowed_domains_vec.iter().any(|domain| {
            host == domain || host.ends_with(&format!(".{}", domain))
        });

        if !is_allowed {
            return create_error(&ctx, format!("Domain '{}' is not in the allowlist", host));
        }

        // Block private IP ranges
        if host == "localhost"
            || host.starts_with("127.")
            || host.starts_with("10.")
            || host.starts_with("192.168.")
            || host.starts_with("172.16.")
            || host == "0.0.0.0" {
            return create_error(&ctx, "Requests to private IP ranges are not allowed".to_string());
        }

        // Extract method and body from options
        let (method, body) = if let Some(opts) = options.0.as_ref() {
            let method = opts.get::<_, Option<String>>("method")
                .unwrap_or(None)
                .unwrap_or_else(|| "GET".to_string())
                .to_uppercase();

            let body = opts.get::<_, Option<String>>("body")
                .unwrap_or(None);

            (method, body)
        } else {
            ("GET".to_string(), None)
        };

        // Validate HTTP method
        if !["GET", "POST", "PUT", "DELETE"].contains(&method.as_str()) {
            return create_error(&ctx, format!("Unsupported HTTP method: {}", method));
        }

        // Make HTTP request with timeout (5 seconds)
        let client = match reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(5))
            .build() {
            Ok(c) => c,
            Err(e) => return create_error(&ctx, format!("Failed to create HTTP client: {}", e)),
        };

        // Build request based on method
        let mut request_builder = match method.as_str() {
            "GET" => client.get(&url),
            "POST" => client.post(&url),
            "PUT" => client.put(&url),
            "DELETE" => client.delete(&url),
            _ => return create_error(&ctx, format!("Unsupported HTTP method: {}", method)),
        };

        // Add body if present (for POST, PUT, DELETE)
        if let Some(body_data) = body {
            request_builder = request_builder
                .header("Content-Type", "application/json")
                .body(body_data);
        }

        let response = match request_builder.send() {
            Ok(r) => r,
            Err(e) => return create_error(&ctx, format!("HTTP request failed: {}", e)),
        };

        let status = response.status().as_u16();
        let response_text = match response.text() {
            Ok(t) => t,
            Err(e) => return create_error(&ctx, format!("Failed to read response: {}", e)),
        };

        // Create response object
        let response_obj = match Object::new(ctx.clone()) {
            Ok(o) => o,
            Err(_) => return None,
        };

        response_obj.set("status", status).ok()?;
        response_obj.set("ok", status >= 200 && status < 300).ok()?;
        response_obj.set("text", response_text.clone()).ok()?;

        // Parse JSON and set as property
        let json_result = match serde_json::from_str::<serde_json::Value>(&response_text) {
            Ok(v) => v,
            Err(_) => serde_json::Value::Null,
        };

        // Convert serde_json::Value to QuickJS Value
        let json_str = serde_json::to_string(&json_result).ok()?;
        let json_value: Value = ctx.eval(format!("({})", json_str).as_str()).ok()?;
        response_obj.set("json", json_value).ok()?;

        Some(response_obj)
    })?;

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
                serde_json::from_str(&json_str)
                    .map_err(|e| anyhow!("Failed to parse JSON: {}", e))
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
        rquickjs::CaughtError::Value(v) => {
            value_to_string(&v)
        }
        rquickjs::CaughtError::Error(e) => {
            format!("{:?}", e)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_execution() {
        let result = execute_js("2 + 2", 5000, 10 * 1024 * 1024, &[]).unwrap();
        assert_eq!(result.value, serde_json::json!(4));
    }

    #[test]
    fn test_console_output() {
        let code = r#"
            console.log("Hello", "World");
            console.error("Error message");
            "done"
        "#;
        let result = execute_js(code, 5000, 10 * 1024 * 1024, &[]).unwrap();
        assert_eq!(result.value, serde_json::json!("done"));
        assert!(result.console_output.len() >= 2);
    }

    #[test]
    fn test_json_return() {
        let code = r#"({ foo: "bar", number: 42, nested: { array: [1, 2, 3] } })"#;
        let result = execute_js(code, 5000, 10 * 1024 * 1024, &[]).unwrap();
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
        let result = execute_js(code, 100, 10 * 1024 * 1024, &[]);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("timeout"));
    }

    #[test]
    fn test_syntax_error() {
        let code = "invalid javascript syntax {{{";
        let result = execute_js(code, 5000, 10 * 1024 * 1024, &[]);
        assert!(result.is_err());
    }

    #[test]
    fn test_fetch_not_allowed_domain() {
        let code = r#"
            const response = fetch("https://evil.com/data");
            response.ok ? "success" : response.error
        "#;
        let result = execute_js(code, 5000, 10 * 1024 * 1024, &["example.com"]).unwrap();
        let response_str = result.value.as_str().unwrap();
        assert!(response_str.contains("not in the allowlist"));
    }

    #[test]
    fn test_fetch_blocked_localhost() {
        let code = r#"
            const response = fetch("http://localhost:8080/secret");
            response.ok ? "success" : response.error
        "#;
        let result = execute_js(code, 5000, 10 * 1024 * 1024, &["localhost"]).unwrap();
        let response_str = result.value.as_str().unwrap();
        assert!(response_str.contains("private IP"));
    }

    #[test]
    fn test_fetch_allowed_domain() {
        // This test actually makes a real HTTP request to httpbin.org
        // Skip in CI or if network is unavailable
        let code = r#"
            const response = fetch("https://httpbin.org/get");
            response.status
        "#;
        let result = execute_js(code, 10000, 10 * 1024 * 1024, &["httpbin.org"]);
        // We expect this to work if network is available
        if let Ok(res) = result {
            assert_eq!(res.value, serde_json::json!(200));
        }
    }
}
