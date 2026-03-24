use serde::{Deserialize, Serialize};

use crate::document::Document;

pub type Task = Document<TaskSpec>;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TaskSpec {
    #[serde(default)]
    pub retry_count: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_seconds: Option<u64>,
    #[serde(default)]
    pub inputs: Vec<TaskParam>,
    #[serde(default)]
    pub outputs: Vec<TaskParam>,
    pub execution: ExecutionSpec,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskParam {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default)]
    pub required: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExecutionSpec {
    pub kind: ExecutionKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
    #[serde(default)]
    pub config: serde_json::Value,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionKind {
    #[default]
    Shell,
    Container,
    Kubernetes,
    Http,
    Sql,
    #[serde(other)]
    Unknown,
}
