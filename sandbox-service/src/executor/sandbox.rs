use std::time::Duration;

use crate::error::AppError;
use mlua::{HookTriggers, Lua, MultiValue, Value, VmState};
use std::sync::mpsc;
use tokio::{task, time};
pub fn start_lua_sandbox() -> mlua::Result<Lua> {
    let lua = Lua::new();

    lua.set_memory_limit(8 * 1024 * 1024)?;

    let globals = lua.globals();
    globals.set("os", Value::Nil)?;
    globals.set("io", Value::Nil)?;
    globals.set("package", Value::Nil)?;
    globals.set("debug", Value::Nil)?;

    Ok(lua)
}

pub async fn execute_lua_code(
    code: String,
    timeout_secs: u64,
) -> std::result::Result<Option<String>, String> {
    let timeout = Duration::from_secs(timeout_secs);

    let execution = task::spawn_blocking(move || {
        let lua = start_lua_sandbox().map_err(|error| error.to_string())?;
        let (transmitter, receiver) = mpsc::channel::<Vec<String>>();
        let start = std::time::Instant::now();
        lua.set_hook(
            HookTriggers {
                every_nth_instruction: Some(1_000),
                ..Default::default()
            },
            move |_lua, _debug| {
                if start.elapsed() > timeout {
                    return Err(mlua::Error::RuntimeError("Execution timed out".to_string()));
                }
                Ok(VmState::Continue)
            },
        );
        let values = lua
            .load(&code)
            .eval::<MultiValue>()
            .map_err(|error| error.to_string())?;

        if values.is_empty() {
            Ok(None)
        } else {
            Ok(Some(format_values(&values)))
        }
    });

    match time::timeout(timeout + Duration::from_millis(100), execution).await {
        Ok(Ok(result)) => result,
        Ok(Err(error)) => Err(format!("runtime task failed: {error}")),
        Err(_) => Err(format!("runtime timed out after {} seconds", timeout_secs)),
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
