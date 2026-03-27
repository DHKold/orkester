use serde::{Deserialize, Serialize};

use crate::catalog::{ExecutionSpec, TaskInputSource, WorkOutputSource};
use crate::document::Document;

pub const TASK_RUN_REQUEST_KIND: &str = "workaholic/TaskRunRequest:1.0";

pub type TaskRunRequestDoc = Document<TaskRunRequestSpec>;

/// A fully-resolved task execution request produced by the scheduler when planning a WorkRun.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskRunRequestSpec {
    /// Reference to the parent Work definition.
    #[serde(rename = "workRef")]
    pub work_ref: String,
    /// Reference to the Task definition being executed.
    #[serde(rename = "taskRef")]
    pub task_ref: String,
    /// Reference to the parent WorkRunRequest.
    #[serde(rename = "workRunRequestRef")]
    pub work_run_request_ref: String,
    /// Name of the step within the Work this request corresponds to.
    #[serde(rename = "stepName")]
    pub step_name: String,
    /// Resolved inputs for this task execution.
    #[serde(default)]
    pub inputs: Vec<ResolvedInput>,
    /// Resolved outputs for this task execution.
    #[serde(default)]
    pub outputs: Vec<ResolvedOutput>,
    /// Execution specification: runner kind, profile, and runner-specific config.
    pub execution: ExecutionSpec,
}

/// A resolved task input with a concrete data source bound at planning time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedInput {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub input_type: Option<String>,
    #[serde(default)]
    pub required: bool,
    /// Concrete source: literal value or artifact URI.
    pub from: TaskInputSource,
}

/// A resolved task output with a concrete destination bound at planning time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedOutput {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub output_type: Option<String>,
    /// Concrete destination: artifact URI (with optional retention) or variable name.
    pub to: WorkOutputSource,
}
