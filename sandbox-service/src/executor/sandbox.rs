use std::time::Duration;
use mlua::{Error as LuaError, HookTriggers, Lua, MultiValue, Value, VmState};
use tokio::sync::mpsc;
use tokio::{task, time};

pub struct ExecutionResult {
    pub output: Option<String>,
    pub logs: Vec<String>,
    pub error: Option<StructuredError>,
    pub memory_used_bytes: Option<u64>,
    pub execution_time_ms: Option<u64>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StructuredError {
    pub kind: ErrorKind,
    pub message: String,
    pub line: Option<u32>,
    pub raw: String,
    pub snippet: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorKind {
    SyntaxError,
    SafetyError,
    RuntimeError,
    Timeout,
    MemoryLimit,
    StackOverflow,
    ForbiddenAccess,
    Unknown,
}

fn parse_lua_error(error: &LuaError, code: &str) -> StructuredError {
    let raw = error.to_string();
    let (line, message) = extract_line_and_message(&raw);
    let snippet = line.map(|l| extract_snippet(code, l));
    let kind = classify_error(&raw);

    StructuredError {
        kind,
        message: message.unwrap_or_else(|| raw.clone()),
        line,
        raw,
        snippet,
    }
}

fn extract_line_and_message(raw: &str) -> (Option<u32>, Option<String>) {
    for part in raw.split(':') {
        if let Ok(line_num) = part.trim().parse::<u32>() {
            let after = raw
                .splitn(3, ':')
                .nth(2)
                .map(|s| s.trim().to_string());
            return (Some(line_num), after);
        }
    }
    (None, None)
}

fn classify_error(raw: &str) -> ErrorKind {
    let lower = raw.to_lowercase();
    if lower.contains("timed out") || lower.contains("execution timed out") {
        ErrorKind::Timeout
    } else if lower.contains("not enough memory") || lower.contains("memory limit") {
        ErrorKind::MemoryLimit
    } else if lower.contains("stack overflow") || lower.contains("c stack overflow") {
        ErrorKind::StackOverflow
    } else if lower.contains("attempt to index a nil value")
        && (lower.contains("global 'os'")
            || lower.contains("global 'io'")
            || lower.contains("global 'debug'")
            || lower.contains("global 'package'"))
    {
        ErrorKind::ForbiddenAccess
    } else if lower.contains("runtime error") || lower.contains("attempt to") {
        ErrorKind::RuntimeError
    } else {
        ErrorKind::Unknown
    }
}

fn extract_snippet(code: &str, error_line: u32) -> String {
    let lines: Vec<&str> = code.lines().collect();
    let total = lines.len() as u32;
    let center = error_line.saturating_sub(1) as usize;
    let start = center.saturating_sub(2);
    let end = (center + 2).min(total.saturating_sub(1) as usize);

    lines[start..=end]
        .iter()
        .enumerate()
        .map(|(i, line)| {
            let lineno = start + i + 1;
            if lineno == error_line as usize {
                format!(">>> {lineno:3} | {line}")  
            } else {
                format!("    {lineno:3} | {line}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn start_lua_sandbox(tx: mpsc::Sender<String>) -> mlua::Result<Lua> {
    let lua = Lua::new();
    lua.set_memory_limit(8 * 1024 * 1024)?;

    let globals = lua.globals();
    globals.set("os", Value::Nil)?;
    globals.set("io", Value::Nil)?;
    globals.set("package", Value::Nil)?;
    globals.set("debug", Value::Nil)?;
    globals.set("coroutine", Value::Nil)?;

    let tx_print = tx.clone();
    let print_fn = lua.create_function(move |_, args: MultiValue| {
        let msg = args.iter().map(format_value).collect::<Vec<_>>().join("\t");
        let _ = tx_print.try_send(format!("[stdout] {msg}"));
        Ok(MultiValue::new())
    })?;
    globals.set("print", print_fn)?;

    let tx_warn = tx.clone();
    let warn_fn = lua.create_function(move |_, args: MultiValue| {
        let msg = args.iter().map(format_value).collect::<Vec<_>>().join("\t");
        let _ = tx_warn.try_send(format!("[warn] {msg}"));
        Ok(MultiValue::new())
    })?;
    globals.set("warn", warn_fn)?;

    Ok(lua)
}

pub async fn execute_lua_code(
    code: String,
    timeout_secs: u64,
) -> ExecutionResult {
    let timeout = Duration::from_secs(timeout_secs);
    let (tx, mut rx) = mpsc::channel::<String>(128);

    let _ = tx
        .send(format!("[exec] starting, timeout={}s", timeout_secs))
        .await;

    let execution = task::spawn_blocking(move || {
        let exec_start = std::time::Instant::now();
        let lua = match start_lua_sandbox(tx.clone()) {
            Ok(lua) => lua,
            Err(e) => {
                let _ = tx.try_send(format!("[fatal] sandbox init failed: {e}"));
                return Err((
                    StructuredError {
                        kind: ErrorKind::Unknown,
                        message: format!("sandbox init failed: {e}"),
                        line: None,
                        raw: e.to_string(),
                        snippet: None,
                    },
                    exec_start.elapsed(),
                    0u64,
                ));
            }
        };

        let start = std::time::Instant::now();
        let tx_hook = tx.clone();

        lua.set_hook(
            HookTriggers {
                every_nth_instruction: Some(10_000),
                ..Default::default()
            },
            move |_, _| {
                if start.elapsed() > timeout {
                    let _ = tx_hook.try_send("[exec] hook: timed out".to_string());
                    return Err(LuaError::RuntimeError("Execution timed out".to_string()));
                }
                Ok(VmState::Continue)
            },
        );

        let _ = tx.try_send(format!("[exec] code size: {} bytes", code.len()));

        match lua.load(&code).eval::<MultiValue>() {
            Ok(values) => {
                let memory = lua.used_memory();
                let elapsed = exec_start.elapsed();
                let _ = tx.try_send(format!("[exec] memory used: {memory} bytes"));
                let output = if values.is_empty() {
                    None
                } else {
                    Some(format_values(&values))
                };
                drop(tx);
                Ok((output, elapsed, memory))
            }
            Err(lua_err) => {
                let structured = parse_lua_error(&lua_err, &code);
                let elapsed = exec_start.elapsed();
                let memory = lua.used_memory();
                let _ = tx.try_send(format!(
                    "[error] kind={:?} line={:?} msg={}",
                    structured.kind, structured.line, structured.message
                ));
                if let Some(ref snippet) = structured.snippet {
                    let _ = tx.try_send(format!("[error] snippet:\n{snippet}"));
                }
                drop(tx);
                Err((structured, elapsed, memory as u64))
            }
        }
    });

    let mut logs = Vec::new();
    while let Some(msg) = rx.recv().await {
        logs.push(msg);
    }

    match time::timeout(timeout + Duration::from_millis(200), execution).await {
        Ok(Ok(Ok((output, elapsed, memory)))) => ExecutionResult {
            output,
            logs,
            error: None,
            memory_used_bytes: Some(memory as u64),
            execution_time_ms: Some(elapsed.as_millis() as u64),
        },
        Ok(Ok(Err((structured_err, elapsed, memory)))) => {
            ExecutionResult {
                output: None,
                logs,
                error: Some(structured_err),
                memory_used_bytes: Some(memory),
                execution_time_ms: Some(elapsed.as_millis() as u64),
            }
        }
        Ok(Err(join_err)) => ExecutionResult {
            output: None,
            logs,
            error: Some(StructuredError {
                kind: ErrorKind::Unknown,
                message: format!("task panicked: {join_err}"),
                line: None,
                raw: join_err.to_string(),
                snippet: None,
            }),
            memory_used_bytes: None,
            execution_time_ms: None,
        },
        Err(_) => ExecutionResult {
            output: None,
            logs,
            error: Some(StructuredError {
                kind: ErrorKind::Timeout,
                message: format!("hard timeout after {timeout_secs}s"),
                line: None,
                raw: "tokio timeout".to_string(),
                snippet: None,
            }),
            memory_used_bytes: None,
            execution_time_ms: Some((timeout + Duration::from_millis(200)).as_millis() as u64),
        },
    }
}
fn format_values(values: &MultiValue) -> String {
    values
        .iter()
        .map(format_value)
        .collect::<Vec<_>>()
        .join("\t")
}

fn format_value(value: &Value) -> String {
    match value {
        Value::Nil => "nil".to_string(),
        Value::Boolean(value) => value.to_string(),
        Value::Integer(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        Value::String(value) => value
            .to_str()
            .map(|value| value.to_string())
            .unwrap_or_else(|_| String::from_utf8_lossy(&value.as_bytes()).into_owned()),
        Value::Function(_) => "<function>".to_string(),
        Value::Thread(_) => "<thread>".to_string(),
        Value::UserData(_) => "<userdata>".to_string(),
        Value::LightUserData(_) => "<lightuserdata>".to_string(),
        Value::Error(error) => error.to_string(),
        Value::Other(_) => "<other>".to_string(),
        Value::Table(t) => {
            let mut parts = vec![];
            let mut is_array = true;
            let mut i = 1;
            loop {
                match t.get::<Value>(i) {
                    Ok(Value::Nil) => break,
                    Ok(v) => {
                        parts.push(format_value(&v));
                        i += 1;
                    }
                    Err(_) => {
                        is_array = false;
                        break;
                    }
                }
            }
            if is_array && !parts.is_empty() {
                format!("[{}]", parts.join(", "))
            } else {
                let mut kv = vec![];
                for pair in t.clone().pairs::<Value, Value>() {
                    if let Ok((k, v)) = pair {
                        kv.push(format!("{}: {}", format_value(&k), format_value(&v)));
                    }
                }
                format!("{{{}}}", kv.join(", "))
            }
        }
    }
}
