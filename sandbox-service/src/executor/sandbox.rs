use std::time::Duration;

use mlua::{HookTriggers, Lua, MultiValue, Value, VmState};
use tokio::sync::mpsc;
use tokio::{task, time};

pub struct ExecutionResult {
    pub output: Option<String>,
    pub logs: Vec<String>,
}

pub fn start_lua_sandbox(tx: mpsc::Sender<String>) -> mlua::Result<Lua> {
    let lua = Lua::new();

    lua.set_memory_limit(8 * 1024 * 1024)?;

    let globals = lua.globals();
    globals.set("os", Value::Nil)?;
    globals.set("io", Value::Nil)?;
    globals.set("package", Value::Nil)?;
    globals.set("debug", Value::Nil)?;

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

pub async fn execute_lua_code(code: String, timeout_secs: u64) -> Result<ExecutionResult, String> {
    let timeout = Duration::from_secs(timeout_secs);
    let (tx, mut rx) = mpsc::channel::<String>(128);

    let _ = tx
        .send(format!(
            "[exec] starting execution, timeout={}s",
            timeout_secs
        ))
        .await;

    let execution = task::spawn_blocking(move || {
        let lua = start_lua_sandbox(tx.clone()).map_err(|error| error.to_string())?;

        let start = std::time::Instant::now();
        let tx_hook = tx.clone();

        lua.set_hook(
            HookTriggers {
                every_nth_instruction: Some(1_000),
                ..Default::default()
            },
            move |_, _debug| {
                if start.elapsed() > timeout {
                    let _ = tx_hook.try_send("[exec] execution timed out".to_string());
                    return Err(mlua::Error::RuntimeError("Execution timed out".to_string()));
                }
                Ok(VmState::Continue)
            },
        );

        let code_len = code.len();
        let _ = tx.try_send(format!("[exec] code size: {} bytes", code_len));

        let values = lua.load(&code).eval::<MultiValue>().map_err(|error| {
            let _ = tx.try_send(format!("[error] {}", error));
            error.to_string()
        })?;

        let memory = lua.used_memory();
        let _ = tx.try_send(format!("[exec] memory used: {} bytes", memory));

        let output = if values.is_empty() {
            None
        } else {
            Some(format_values(&values))
        };

        drop(tx);
        Ok::<_, String>(output)
    });

    let mut logs = Vec::new();
    while let Some(msg) = rx.recv().await {
        logs.push(msg);
    }

    let result = match time::timeout(timeout + Duration::from_millis(100), execution).await {
        Ok(Ok(Ok(output))) => Ok(output),
        Ok(Ok(Err(error))) => Err(error),
        Ok(Err(error)) => Err(format!("runtime task failed: {error}")),
        Err(_) => Err(format!("runtime timed out after {} seconds", timeout_secs)),
    };

    match result {
        Ok(output) => Ok(ExecutionResult { output, logs }),
        Err(error) => {
            logs.push(format!("[fatal] {error}"));
            Ok(ExecutionResult { output: None, logs })
        }
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
        Value::Table(_) => "<table>".to_string(),
        Value::Function(_) => "<function>".to_string(),
        Value::Thread(_) => "<thread>".to_string(),
        Value::UserData(_) => "<userdata>".to_string(),
        Value::LightUserData(_) => "<lightuserdata>".to_string(),
        Value::Error(error) => error.to_string(),
        Value::Other(_) => "<other>".to_string(),
    }
}
