use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct PipelineRequest {
    pub code: String,
    pub execute: Option<bool>,
    pub timeout: Option<u64>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PipelineResponse {
    pub ok: bool,
    pub syntax_ok: bool,
    pub safety_ok: bool,
    pub runtime_ok: bool,
    pub stage: String,
    pub error: Option<String>,
    pub output: Option<String>,
    pub warnings: Vec<String>,
}
