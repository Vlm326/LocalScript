use tree_sitter::Parser;
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
