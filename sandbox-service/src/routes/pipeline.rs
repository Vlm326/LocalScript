use axum::Json;

use crate::ast::extractor::extract_function_calls;
use crate::ast::parser::{parse_lua_code, recursive_ast_walk};
use crate::ast::safety::{find_dangerous_text_patterns, find_forbidden_ast_calls};
use crate::executor::sandbox::execute_lua_code;
use crate::executor::sandbox::{ErrorKind, StructuredError};
use crate::models::{
    AstAnalysis, ExecutionStats, PipelineRequest, PipelineResponse, PipelineStatus,
};

fn make_error_detail(
    kind: ErrorKind,
    message: &str,
    code: &str,
    line: Option<u32>,
) -> StructuredError {
    let snippet = line.map(|l| {
        let lines: Vec<&str> = code.lines().collect();
        let center = (l as usize).saturating_sub(1);
        let start = center.saturating_sub(2);
        let end = (center + 2).min(lines.len().saturating_sub(1));
        lines[start..=end]
            .iter()
            .enumerate()
            .map(|(i, line)| {
                let lineno = start + i + 1;
                if lineno == l as usize {
                    format!(">>> {lineno:3} | {line}")
                } else {
                    format!("    {lineno:3} | {line}")
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    });
    StructuredError {
        kind,
        message: message.to_string(),
        line,
        raw: message.to_string(),
        snippet,
    }
}

pub async fn handle_pipeline(Json(payload): Json<PipelineRequest>) -> Json<PipelineResponse> {
    let should_execute = payload.execute.unwrap_or(false);
    let timeout_secs = payload.timeout.unwrap_or(2).clamp(1, 10);
    let mut warnings = Vec::new();
    let source_code = payload.code.clone();

    if payload
        .timeout
        .is_some_and(|timeout| !(1..=10).contains(&timeout))
    {
        warnings.push(format!("timeout normalized to {} seconds", timeout_secs));
    }

    // --- Stage: Parse ---
    if payload.code.trim().is_empty() {
        return Json(PipelineResponse {
            status: PipelineStatus::SyntaxError,
            source_code,
            output: None,
            logs: Vec::new(),
            warnings,
            error_detail: Some(make_error_detail(
                ErrorKind::SyntaxError,
                "code is empty",
                &payload.code,
                None,
            )),
            ast_analysis: None,
            execution_stats: None,
        });
    }

    let tree = match parse_lua_code(&payload.code) {
        Ok(tree) => tree,
        Err(error) => {
            return Json(PipelineResponse {
                status: PipelineStatus::SyntaxError,
                source_code,
                output: None,
                logs: Vec::new(),
                warnings,
                error_detail: Some(make_error_detail(
                    ErrorKind::SyntaxError,
                    &error,
                    &payload.code,
                    None,
                )),
                ast_analysis: None,
                execution_stats: None,
            });
        }
    };

    let mut ast_errors = Vec::new();
    recursive_ast_walk(tree.root_node(), &mut ast_errors, &payload.code);

    if let Some(first_error) = ast_errors.first() {
        return Json(PipelineResponse {
            status: PipelineStatus::SyntaxError,
            source_code,
            output: None,
            logs: Vec::new(),
            warnings,
            error_detail: Some(StructuredError {
                kind: ErrorKind::SyntaxError,
                message: format!(
                    "Syntax error at line {}, column {}",
                    first_error.line, first_error.column
                ),
                line: Some(first_error.line as u32),
                raw: "Syntax error in AST".to_string(),
                snippet: Some(first_error.snippet.clone()),
            }),
            ast_analysis: None,
            execution_stats: None,
        });
    }

    // --- Stage: Safety ---
    let has_dangerous = find_dangerous_text_patterns(&payload.code);
    if let Some(ref matches) = has_dangerous {
        let error_message = matches.join(", ");
        return Json(PipelineResponse {
            status: PipelineStatus::SafetyError,
            source_code,
            output: None,
            logs: Vec::new(),
            warnings,
            error_detail: Some(make_error_detail(
                ErrorKind::SafetyError,
                &error_message,
                &payload.code,
                None,
            )),
            ast_analysis: Some(AstAnalysis {
                function_calls: extract_function_calls(&tree, &payload.code),
                has_dangerous_patterns: true,
                has_forbidden_calls: false,
            }),
            execution_stats: None,
        });
    }

    let calls = extract_function_calls(&tree, &payload.code);
    let has_forbidden = find_forbidden_ast_calls(&calls);
    if let Some(ref matches) = has_forbidden {
        let error_message = matches.join(", ");
        return Json(PipelineResponse {
            status: PipelineStatus::SafetyError,
            source_code,
            output: None,
            logs: Vec::new(),
            warnings,
            error_detail: Some(make_error_detail(
                ErrorKind::SafetyError,
                &error_message,
                &payload.code,
                None,
            )),
            ast_analysis: Some(AstAnalysis {
                function_calls: calls,
                has_dangerous_patterns: false,
                has_forbidden_calls: true,
            }),
            execution_stats: None,
        });
    }

    // --- Stage: Skip Execution ---
    if !should_execute {
        warnings.push("runtime execution skipped".to_string());
        return Json(PipelineResponse {
            status: PipelineStatus::Ok,
            source_code,
            output: None,
            logs: Vec::new(),
            warnings,
            error_detail: None,
            ast_analysis: Some(AstAnalysis {
                function_calls: calls,
                has_dangerous_patterns: false,
                has_forbidden_calls: false,
            }),
            execution_stats: None,
        });
    }

    // --- Stage: Execute ---
    let context = payload.context.clone().unwrap_or_else(|| {
        serde_json::json!({
            "wf": {
                "vars": {},
                "initVariables": {}
            }
        })
    });
    let result = execute_lua_code(payload.code, timeout_secs, context).await;

    let execution_stats = Some(ExecutionStats {
        memory_used_bytes: result.memory_used_bytes,
        execution_time_ms: result.execution_time_ms,
    });

    let ast_analysis = Some(AstAnalysis {
        function_calls: calls,
        has_dangerous_patterns: false,
        has_forbidden_calls: false,
    });

    if let Some(err) = result.error {
        let status = match err.kind {
            ErrorKind::Timeout => PipelineStatus::Timeout,
            _ => PipelineStatus::RuntimeError,
        };
        return Json(PipelineResponse {
            status,
            source_code,
            output: None,
            logs: result.logs,
            warnings,
            error_detail: Some(err),
            ast_analysis,
            execution_stats,
        });
    }

    Json(PipelineResponse {
        status: PipelineStatus::Ok,
        source_code,
        output: result.output,
        logs: result.logs,
        warnings,
        error_detail: None,
        ast_analysis,
        execution_stats,
    })
}
