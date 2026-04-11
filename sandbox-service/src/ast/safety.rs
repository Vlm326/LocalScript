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

const FORBIDDEN_AST_CALLS: [&str; 7] = [
    "os.execute",
    "io.open",
    "io.popen",
    "require",
    "dofile",
    "loadfile",
    "package.loadlib",
];

pub fn find_forbidden_ast_calls(calls: &[String]) -> Option<Vec<String>> {
    let mut matches = Vec::new();

    for call in calls {
        if FORBIDDEN_AST_CALLS.contains(&call.as_str()) {
            matches.push(format!("forbidden function call found: {call}"));
        }
    }

    if matches.is_empty() {
        None
    } else {
        Some(matches)
    }
}
