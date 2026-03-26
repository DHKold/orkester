use thiserror::Error;

#[derive(Debug, Error)]
pub enum WorkaholicError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("YAML parse error: {0}")]
    Yaml(#[from] serde_yaml::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Persistence error: {0}")]
    Persistence(String),

    #[error("Not found: {kind} '{name}'")]
    NotFound { kind: String, name: String },

    #[error("Already exists: {kind} '{name}'")]
    AlreadyExists { kind: String, name: String },

    #[error("Invalid document: {0}")]
    InvalidDocument(String),

    #[error("DAG error: {0}")]
    Dag(String),

    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, WorkaholicError>;
