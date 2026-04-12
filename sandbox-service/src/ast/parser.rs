use crate::models::AstFragment;
use std::sync::{LazyLock, Mutex};
use tree_sitter::{Node, Parser};
use tree_sitter_lua::LANGUAGE;

static LUA_PARSER: LazyLock<Mutex<Parser>> = LazyLock::new(|| {
    let mut parser = Parser::new();
    parser
        .set_language(&LANGUAGE.into())
        .expect("Failed to set Lua language");
    Mutex::new(parser)
});

pub fn parse_lua_code(code: &str) -> Result<tree_sitter::Tree, String> {
    let mut parser = LUA_PARSER.lock().map_err(|e| e.to_string())?;
    parser
        .parse(code, None)
        .ok_or_else(|| "Failed to parse code".to_string())
}
pub fn recursive_ast_walk(node: Node, errors: &mut Vec<AstFragment>, source: &str) {
    for index in 0..node.child_count() {
        let Some(child) = node.child(index) else {
            continue;
        };

        if child.is_error() || child.is_missing() {
            let pos = child.start_position();
            errors.push(AstFragment {
                line: pos.row + 1,
                column: pos.column + 1,
                snippet: extract_snippet(source, pos.row + 1),
            });
        }

        recursive_ast_walk(child, errors, source);
    }
}

fn extract_snippet(code: &str, error_line: usize) -> String {
    let lines: Vec<&str> = code.lines().collect();
    if lines.is_empty() {
        return String::new();
    }

    let center = error_line.saturating_sub(1).min(lines.len() - 1);
    let start = center.saturating_sub(2);
    let end = (center + 2).min(lines.len() - 1);

    lines[start..=end]
        .iter()
        .enumerate()
        .map(|(i, line)| {
            let lineno = start + i + 1;
            if lineno == error_line {
                format!(">>> {lineno:3} | {line}")
            } else {
                format!("    {lineno:3} | {line}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}
