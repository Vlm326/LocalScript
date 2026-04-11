use axum::Json;

use crate::ast::extractor::extract_function_calls;
use crate::ast::parser::{parse_lua_code, recursive_ast_walk};
use crate::ast::safety::{find_dangerous_text_patterns, find_forbidden_ast_calls};
use crate::executor::sandbox::execute_lua_code;
use crate::models::{PipelineRequest, PipelineResponse};

pub fn construct_pipeline_response(
    ok: bool,
    syntax_ok: bool,
    safety_ok: bool,
    runtime_ok: bool,
    stage: &str,
    error: Option<String>,
    output: Option<String>,
    warnings: Vec<String>,
) -> PipelineResponse {
    PipelineResponse {
        ok,
        syntax_ok,
        safety_ok,
        runtime_ok,
        stage: stage.to_string(),
        error,
        output,
        warnings,
    }
}

pub async fn handle_pipeline(Json(payload): Json<PipelineRequest>) -> Json<PipelineResponse> {
    let should_execute = payload.execute.unwrap_or(false);
    let timeout_secs = payload.timeout.unwrap_or(2).clamp(1, 10);
    let mut warnings = Vec::new();

    if payload
        .timeout
        .is_some_and(|timeout| !(1..=10).contains(&timeout))
    {
        warnings.push(format!("timeout normalized to {} seconds", timeout_secs));
    }

    if payload.code.trim().is_empty() {
        return Json(construct_pipeline_response(
            false,
            false,
            false,
            false,
            "parse",
            Some("code is empty".to_string()),
            None,
            warnings,
        ));
    }

    let tree = match parse_lua_code(&payload.code) {
        Ok(tree) => tree,
        Err(error) => {
            return Json(construct_pipeline_response(
                false,
                false,
                false,
                false,
                "parse",
                Some(error),
                None,
                warnings,
            ));
        }
    };

    let mut ast_errors = Vec::new();
    recursive_ast_walk(tree.root_node(), &mut ast_errors);

    if !ast_errors.is_empty() {
        return Json(construct_pipeline_response(
            false,
            false,
            false,
            false,
            "parse",
            Some(ast_errors.join(", ")),
            None,
            warnings,
        ));
    }

    if let Some(matches) = find_dangerous_text_patterns(&payload.code) {
        return Json(construct_pipeline_response(
            false,
            true,
            false,
            false,
            "safety",
            Some(matches.join(", ")),
            None,
            warnings,
        ));
    }

    let calls = extract_function_calls(&tree, &payload.code);

    if let Some(matches) = find_forbidden_ast_calls(&calls) {
        return Json(construct_pipeline_response(
            false,
            true,
            false,
            false,
            "safety",
            Some(matches.join(", ")),
            None,
            warnings,
        ));
    }

    if !should_execute {
        warnings.push("runtime execution skipped".to_string());
        return Json(construct_pipeline_response(
            true,
            true,
            true,
            true,
            "completed",
            None,
            None,
            warnings,
        ));
    }

    match execute_lua_code(payload.code, timeout_secs).await {
        Ok(output) => Json(construct_pipeline_response(
            true,
            true,
            true,
            true,
            "completed",
            None,
            output,
            warnings,
        )),
        Err(error) => Json(construct_pipeline_response(
            false,
            true,
            true,
            false,
            "runtime",
            Some(error),
            None,
            warnings,
        )),
    }
}
