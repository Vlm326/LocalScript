use axum::Json;

use crate::ast::extractor::extract_function_calls;
use crate::ast::parser::{parse_lua_code, recursive_ast_walk};
use crate::ast::safety::{find_dangerous_text_patterns, find_forbidden_ast_calls};
use crate::executor::sandbox::execute_lua_code;
use crate::executor::sandbox::{ErrorKind, StructuredError};
use crate::models::{
    AstAnalysis, ExecutionStats, PipelineRequest, PipelineResponse, PipelineStatus,
};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;
use tracing::{info, warn};

static REQUEST_ID: AtomicU64 = AtomicU64::new(1);

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
    let started = Instant::now();
    let rid = REQUEST_ID.fetch_add(1, Ordering::Relaxed);

    let should_execute = payload.execute.unwrap_or(false);
    let timeout_secs = payload.timeout.unwrap_or(2).clamp(1, 10);
    let mut warnings = Vec::new();
    let source_code = payload.code.clone();

    info!(
        rid,
        execute = should_execute,
        timeout_secs,
        code_len = payload.code.len(),
        context_present = payload.context.is_some(),
        "pipeline: request_started"
    );
    // User requested full code visibility in logs.
    // This can be noisy, but makes sandbox the single source of truth for what was executed/validated.
    info!(rid, "pipeline: code\n{}", payload.code);

    if payload
        .timeout
        .is_some_and(|timeout| !(1..=10).contains(&timeout))
    {
        warnings.push(format!("timeout normalized to {} seconds", timeout_secs));
        warn!(rid, timeout_secs, "pipeline: timeout_normalized");
    }

    // --- Stage: Parse ---
    info!(rid, "pipeline: parse_started");
    if payload.code.trim().is_empty() {
        let resp = PipelineResponse {
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
        };
        info!(
            rid,
            status = ?resp.status,
            elapsed_ms = started.elapsed().as_millis() as u64,
            "pipeline: response_sent"
        );
        return Json(resp);
    }

    let tree = match parse_lua_code(&payload.code) {
        Ok(tree) => tree,
        Err(error) => {
            let resp = PipelineResponse {
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
            };
            info!(
                rid,
                status = ?resp.status,
                elapsed_ms = started.elapsed().as_millis() as u64,
                "pipeline: response_sent"
            );
            return Json(resp);
        }
    };

    let mut ast_errors = Vec::new();
    recursive_ast_walk(tree.root_node(), &mut ast_errors, &payload.code);

    if let Some(first_error) = ast_errors.first() {
        let resp = PipelineResponse {
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
        };
        info!(
            rid,
            status = ?resp.status,
            elapsed_ms = started.elapsed().as_millis() as u64,
            "pipeline: response_sent"
        );
        return Json(resp);
    }

    // --- Stage: Safety ---
    info!(rid, "pipeline: safety_text_started");
    let has_dangerous = find_dangerous_text_patterns(&payload.code);
    if let Some(ref matches) = has_dangerous {
        let error_message = matches.join(", ");
        let calls = extract_function_calls(&tree, &payload.code);
        let resp = PipelineResponse {
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
                has_dangerous_patterns: true,
                has_forbidden_calls: false,
            }),
            execution_stats: None,
        };
        info!(
            rid,
            status = ?resp.status,
            error_kind = ?resp.error_detail.as_ref().map(|e| &e.kind),
            elapsed_ms = started.elapsed().as_millis() as u64,
            "pipeline: response_sent"
        );
        return Json(resp);
    }

    info!(rid, "pipeline: safety_ast_started");
    let calls = extract_function_calls(&tree, &payload.code);
    let has_forbidden = find_forbidden_ast_calls(&calls);
    if let Some(ref matches) = has_forbidden {
        let error_message = matches.join(", ");
        let resp = PipelineResponse {
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
        };
        info!(
            rid,
            status = ?resp.status,
            error_kind = ?resp.error_detail.as_ref().map(|e| &e.kind),
            elapsed_ms = started.elapsed().as_millis() as u64,
            "pipeline: response_sent"
        );
        return Json(resp);
    }

    // --- Stage: Skip Execution ---
    if !should_execute {
        warnings.push("runtime execution skipped".to_string());
        info!(rid, "pipeline: execution_skipped");
        let resp = PipelineResponse {
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
        };
        info!(
            rid,
            status = ?resp.status,
            warnings_len = resp.warnings.len(),
            elapsed_ms = started.elapsed().as_millis() as u64,
            "pipeline: response_sent"
        );
        return Json(resp);
    }

    // --- Stage: Execute ---
    info!(rid, timeout_secs, "pipeline: execution_started");
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
        let resp = PipelineResponse {
            status,
            source_code,
            output: None,
            logs: result.logs,
            warnings,
            error_detail: Some(err),
            ast_analysis,
            execution_stats,
        };
        info!(
            rid,
            status = ?resp.status,
            error_kind = ?resp.error_detail.as_ref().map(|e| &e.kind),
            logs_len = resp.logs.len(),
            warnings_len = resp.warnings.len(),
            elapsed_ms = started.elapsed().as_millis() as u64,
            "pipeline: response_sent"
        );
        return Json(resp);
    }

    let resp = PipelineResponse {
        status: PipelineStatus::Ok,
        source_code,
        output: result.output,
        logs: result.logs,
        warnings,
        error_detail: None,
        ast_analysis,
        execution_stats,
    };
    info!(
        rid,
        status = ?resp.status,
        output_present = resp.output.is_some(),
        logs_len = resp.logs.len(),
        warnings_len = resp.warnings.len(),
        elapsed_ms = started.elapsed().as_millis() as u64,
        "pipeline: response_sent"
    );
    Json(resp)
}
