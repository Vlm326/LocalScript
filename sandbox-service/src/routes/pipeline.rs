use axum::Json;

use crate::models::{PipelineRequest, PipelineResponse};

pub async fn handle_pipeline(Json(payload): Json<PipelineRequest>) -> Json<PipelineResponse> {
    if payload.code.trim().is_empty() {
        return Json(PipelineResponse {
            ok: false,
            syntax_ok: false,
            safety_ok: false,
            runtime_ok: false,
            stage: "parse".to_string(),
            error: Some("code is empty".to_string()),
            output: None,
            warnings: Vec::new(),
        });
    }

    Json(PipelineResponse {
        ok: true,
        syntax_ok: false,
        safety_ok: false,
        runtime_ok: false,
        stage: "stub".to_string(),
        error: None,
        output: None,
        warnings: vec!["pipeline logic is not implemented yet".to_string()],
    })
}
