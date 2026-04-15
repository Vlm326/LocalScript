use crate::executor::sandbox::StructuredError;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum PipelineStatus {
    Ok,
    SyntaxError,
    SafetyError,
    RuntimeError,
    Timeout,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AstFragment {
    pub line: usize,
    pub column: usize,
    pub snippet: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AstAnalysis {
    pub function_calls: Vec<String>,
    pub has_dangerous_patterns: bool,
    pub has_forbidden_calls: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ExecutionStats {
    pub memory_used_bytes: Option<u64>,
    pub execution_time_ms: Option<u64>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PipelineRequest {
    pub code: String,
    pub execute: Option<bool>,
    pub timeout: Option<u64>,
    pub context: Option<JsonValue>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PipelineResponse {
    pub status: PipelineStatus,
    pub source_code: String,
    pub output: Option<String>,
    pub logs: Vec<String>,
    pub warnings: Vec<String>,
    pub error_detail: Option<StructuredError>,
    pub ast_analysis: Option<AstAnalysis>,
    pub execution_stats: Option<ExecutionStats>,
}
