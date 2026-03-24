use serde::{Deserialize, Serialize};

use crate::document::Document;

pub type Work = Document<WorkSpec>;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorkSpec {
    /// Named parameters accepted by this workflow.
    #[serde(default)]
    pub params: Vec<WorkParam>,
    /// Task nodes that form the workflow DAG.
    #[serde(default)]
    pub tasks: Vec<WorkTask>,
    #[serde(default)]
    pub concurrency: WorkConcurrency,
    #[serde(default)]
    pub failure_policy: WorkFailurePolicy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkParam {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<serde_json::Value>,
    #[serde(default)]
    pub required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkTask {
    /// Node name — unique within the Work definition.
    pub name: String,
    /// Reference to a Task document in `name:version` or `name` format.
    pub task_ref: String,
    /// Names of task nodes this one depends on (DAG edges).
    #[serde(default)]
    pub depends_on: Vec<String>,
    /// Resolved inputs (may reference workflow params via `{{ params.x }}`).
    #[serde(default)]
    pub inputs: serde_json::Value,
    /// Optional guard expression (future use).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub condition: Option<String>,
    /// Override retry count from the Task definition.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retry_count: Option<u32>,
    /// Override timeout from the Task definition (seconds).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_seconds: Option<u64>,
    /// Override execution profile.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub execution_profile: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorkConcurrency {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_parallel_tasks: Option<usize>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorkFailurePolicy {
    #[serde(default)]
    pub mode: FailureMode,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FailureMode {
    #[default]
    FailFast,
    Continue,
}
