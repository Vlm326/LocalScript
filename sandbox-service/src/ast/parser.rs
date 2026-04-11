use tree_sitter::{Node, Parser};
use tree_sitter_lua::LANGUAGE;

pub fn parse_lua_code(code: &str) -> Result<tree_sitter::Tree, String> {
    let mut parser = Parser::new();
    parser
        .set_language(&LANGUAGE.into())
        .map_err(|e| e.to_string())?;

    let tree = parser.parse(code, None).ok_or("Failed to parse code")?;
    let root_node = tree.root_node();

    if root_node.has_error() {
        return Err("Syntax error in code, can't parse AST".to_string());
    }

    Ok(tree)
}

pub fn recursive_ast_walk(node: Node, errors: &mut Vec<String>) {
    for index in 0..node.child_count() {
        let Some(child) = node.child(index) else {
            continue;
        };

        if child.is_error() {
            errors.push(format!(
                "Syntax error at line {}, column {}",
                child.start_position().row + 1,
                child.start_position().column + 1
            ));
        }
        if child.is_missing() {
            errors.push(format!(
                "Missing node at line {}, column {}",
                child.start_position().row + 1,
                child.start_position().column + 1
            ));
        }

        recursive_ast_walk(child, errors);
    }
}
