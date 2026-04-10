use axum::Json;

use crate::ast::parser::parse_lua_code;
use crate::ast::safety::find_dangerous_text_patterns;
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
    if payload.code.trim().is_empty() {
        return Json(construct_pipeline_response(
            false,
            false,
            false,
            false,
            "parse",
            Some("code is empty".to_string()),
            None,
            Vec::new(),
        ));
    }

    if let Err(error) = parse_lua_code(&payload.code) {
        return Json(construct_pipeline_response(
            false,
            false,
            false,
            false,
            "parse",
            Some(error),
            None,
            Vec::new(),
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
            Vec::new(),
        ));
    }

    Json(construct_pipeline_response(
        true,
        true,
        true,
        false,
        "stub",
        None,
        None,
        vec!["pipeline logic is not implemented yet".to_string()],
    ))
}
