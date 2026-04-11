use std::fs::read_to_string;
use thiserror::Error;

/// Стадия выполнения (типобезопасно, без строк)
#[derive(Debug, Clone, Copy)]
pub enum Stage {
    Parsing,
    Execution,
    SafetyCheck,
}

#[derive(Debug, Error)]
pub enum AppError {
    #[error("Parse error at {stage:?}: {message}")]
    Parse { message: String, stage: Stage },

    #[error("Runtime error at {stage:?}: {message}")]
    Runtime { message: String, stage: Stage },

    #[error("Safety error at {stage:?}: {message}")]
    Safety { message: String, stage: Stage },

    #[error("Internal error at {stage:?}: {message}")]
    Internal { message: String, stage: Stage },
}

impl AppError {
    pub fn parse(msg: impl Into<String>) -> Self {
        Self::Parse {
            message: msg.into(),
            stage: Stage::Parsing,
        }
    }

    pub fn runtime(msg: impl Into<String>) -> Self {
        Self::Runtime {
            message: msg.into(),
            stage: Stage::Execution,
        }
    }

    pub fn safety(msg: impl Into<String>) -> Self {
        Self::Safety {
            message: msg.into(),
            stage: Stage::SafetyCheck,
        }
    }

    pub fn internal(msg: impl Into<String>) -> Self {
        Self::Internal {
            message: msg.into(),
            stage: Stage::Execution,
        }
    }
}

impl From<std::io::Error> for AppError {
    fn from(err: std::io::Error) -> Self {
        AppError::Internal {
            message: err.to_string(),
            stage: Stage::Execution,
        }
    }
}
