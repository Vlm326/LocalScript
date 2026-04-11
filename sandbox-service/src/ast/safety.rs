use std::collections::HashSet;
use std::sync::LazyLock;

const DANGEROUS_TEXT_PATTERNS: [&str; 10] = [
    "rm -rf",
    "rm -fr",
    "sudo rm",
    "mkfs",
    "dd if=",
    ":(){",
    "shutdown",
    "reboot",
    "curl | sh",
    "wget | sh",
];

pub fn find_dangerous_text_patterns(code: &str) -> Option<Vec<String>> {
    let normalized_code = code.to_lowercase();
    let mut matches = Vec::new();

    for pattern in DANGEROUS_TEXT_PATTERNS.iter() {
        if normalized_code.contains(pattern) {
            matches.push(format!("dangerous text pattern found: {pattern}"));
        }
    }

    if matches.is_empty() {
        None
    } else {
        Some(matches)
    }
}

const FORBIDDEN_AST_CALLS: [&str; 34] = [
    // OS operations
    "os.execute",
    "os.remove",
    "os.rename",
    "os.tmpname",
    "os.exit",
    // I/O operations
    "io.open",
    "io.popen",
    "io.input",
    "io.output",
    "io.close",
    // Dynamic loading
    "require",
    "dofile",
    "loadfile",
    "load",
    "loadstring",
    "package.loadlib",
    // Debug & metatable bypass
    "debug",
    "setmetatable",
    "getmetatable",
    "getfenv",
    "setfenv",
    "rawget",
    "rawset",
    "rawequal",
    // Global environment access
    "_G",
    "_ENV",
    // Попытки обойти sandbox через строки
    "load(", // load("os.execute...")
    "loadstring(",
    "dostring(",
    // Обфускация через char codes
    "string.char(", // string.char(111,115) = "os"
    "string.byte(",
    // Попытка достать глобальное окружение
    "getfenv(0)", // Lua 5.1 root env
    "debug.getinfo",
    "coroutine.wrap", // может использоваться для обхода hook
];

static FORBIDDEN_SET: LazyLock<HashSet<&str>> =
    LazyLock::new(|| FORBIDDEN_AST_CALLS.iter().copied().collect());

pub fn find_forbidden_ast_calls(calls: &[String]) -> Option<Vec<String>> {
    let mut matches = Vec::new();

    for call in calls {
        // Точное совпадение — O(1) через HashSet
        if FORBIDDEN_SET.contains(call.as_str()) {
            matches.push(format!("forbidden function call found: {call}"));
            continue;
        }

        // Частичное совпадение: проверка префиксов (os.execute, os["execute"] и т.д.)
        for forbidden in &FORBIDDEN_AST_CALLS {
            // Проверяем dot notation: os.execute, io.open
            if call.starts_with(&format!("{forbidden}."))
                || call.starts_with(&format!("{forbidden}:"))
            {
                matches.push(format!("forbidden access via dot/method: {call}"));
                break;
            }

            // Проверяем bracket notation: os["execute"], io["open"]
            if call.starts_with(&format!("{forbidden}[")) {
                matches.push(format!("forbidden access via bracket notation: {call}"));
                break;
            }
        }
    }

    if matches.is_empty() {
        None
    } else {
        Some(matches)
    }
}
