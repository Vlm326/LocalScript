use tree_sitter::{Node, Tree};

pub fn extract_function_calls(tree: &Tree, source: &str) -> Vec<String> {
    let mut calls = Vec::new();
    collect_function_calls(tree.root_node(), source, &mut calls);
    calls
}

fn collect_function_calls(node: Node, source: &str, calls: &mut Vec<String>) {
    if node.kind() == "function_call" {
        if let Some(name_node) = node.child_by_field_name("name") {
            if let Some(call_name) = extract_callable_name(name_node, source) {
                calls.push(call_name);
            }
        }
    }

    for index in 0..node.child_count() {
        if let Some(child) = node.child(index) {
            collect_function_calls(child, source, calls);
        }
    }
}

fn extract_callable_name(node: Node, source: &str) -> Option<String> {
    match node.kind() {
        "identifier" | "global" => node_text(node, source),
        "dot_index_expression" => {
            let table = node.child_by_field_name("table")?;
            let field = node.child_by_field_name("field")?;
            let table_name = extract_callable_name(table, source)?;
            let field_name = node_text(field, source)?;
            Some(format!("{table_name}.{field_name}"))
        }
        "method_index_expression" => {
            let table = node.child_by_field_name("table")?;
            let method = node.child_by_field_name("method")?;
            let table_name = extract_callable_name(table, source)?;
            let method_name = node_text(method, source)?;
            Some(format!("{table_name}:{method_name}"))
        }
        "parenthesized_expression" => {
            for index in 0..node.child_count() {
                if let Some(child) = node.child(index) {
                    if child.is_named() {
                        return extract_callable_name(child, source);
                    }
                }
            }
            None
        }
        "function_call" => node
            .child_by_field_name("name")
            .and_then(|name| extract_callable_name(name, source)),
        _ => None,
    }
}

fn node_text(node: Node, source: &str) -> Option<String> {
    node.utf8_text(source.as_bytes())
        .ok()
        .map(|text| text.to_string())
}
