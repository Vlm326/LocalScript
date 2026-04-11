use axum::Json;

use crate::ast::extractor::extract_function_calls;
use crate::ast::parser::{parse_lua_code, recursive_ast_walk};
use crate::ast::safety::{find_dangerous_text_patterns, find_forbidden_ast_calls};
use crate::error::AppError;
use crate::executor::sandbox::execute_lua_code;
use crate::models::{PipelineRequest, PipelineResponse, PipelineStatus};

pub fn construct_pipeline_response(
    status: PipelineStatus,
    output: Option<String>,
    logs: Vec<String>,
    warnings: Vec<String>,
) -> PipelineResponse {
    PipelineResponse {
        status,
        output,
        logs,
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
            PipelineStatus::SyntaxError {
                error: AppError::parse("code is empty"),
            },
            None,
            Vec::new(),
            warnings,
        ));
    }

    let tree = match parse_lua_code(&payload.code) {
        Ok(tree) => tree,
        Err(error) => {
            return Json(construct_pipeline_response(
                PipelineStatus::SyntaxError {
                    error: AppError::parse(error),
                },
                None,
                Vec::new(),
                warnings,
            ));
        }
    };

    let mut ast_errors = Vec::new();
    recursive_ast_walk(tree.root_node(), &mut ast_errors);

    if !ast_errors.is_empty() {
        let error_message = ast_errors.join(", ");
        return Json(construct_pipeline_response(
            PipelineStatus::SyntaxError {
                error: AppError::parse(error_message),
            },
            None,
            Vec::new(),
            warnings,
        ));
    }

    if let Some(matches) = find_dangerous_text_patterns(&payload.code) {
        let error_message = matches.join(", ");
        return Json(construct_pipeline_response(
            PipelineStatus::SafetyError {
                error: AppError::safety(error_message),
            },
            None,
            Vec::new(),
            warnings,
        ));
    }

    let calls = extract_function_calls(&tree, &payload.code);

    if let Some(matches) = find_forbidden_ast_calls(&calls) {
        let error_message = matches.join(", ");
        return Json(construct_pipeline_response(
            PipelineStatus::SafetyError {
                error: AppError::safety(error_message),
            },
            None,
            Vec::new(),
            warnings,
        ));
    }

    if !should_execute {
        warnings.push("runtime execution skipped".to_string());
        return Json(construct_pipeline_response(
            PipelineStatus::Ok,
            None,
            Vec::new(),
            warnings,
        ));
    }

    let result = match execute_lua_code(payload.code, timeout_secs).await {
        Ok(result) => result,
        Err(error) => {
            return Json(construct_pipeline_response(
                PipelineStatus::RuntimeError {
                    error: AppError::runtime(&error),
                },
                None,
                vec![format!("[fatal] {error}")],
                warnings,
            ));
        }
    };

    let has_runtime_errors = result
        .logs
        .iter()
        .any(|l| l.starts_with("[error]") || l.starts_with("[fatal]"));

    if has_runtime_errors {
        let error_msg = result
            .logs
            .iter()
            .find(|l| l.starts_with("[error]") || l.starts_with("[fatal]"))
            .cloned()
            .unwrap_or_else(|| "unknown runtime error".to_string());

        return Json(construct_pipeline_response(
            PipelineStatus::RuntimeError {
                error: AppError::runtime(&error_msg),
            },
            None,
            result.logs,
            warnings,
        ));
    }

    Json(construct_pipeline_response(
        PipelineStatus::Ok,
        result.output,
        result.logs,
        warnings,
    ))
}
