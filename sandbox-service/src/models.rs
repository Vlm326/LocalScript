use crate::error::AppError;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum PipelineStatus {
    Ok,
    SyntaxError { error: AppError },
    SafetyError { error: AppError },
    RuntimeError { error: AppError },
    Timeout { error: AppError },
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PipelineRequest {
    pub code: String,
    pub execute: Option<bool>,
    pub timeout: Option<u64>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PipelineResponse {
    pub status: PipelineStatus,
    pub output: Option<String>,
    pub logs: Vec<String>,
    pub warnings: Vec<String>,
}
