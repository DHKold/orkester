use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

use crate::document::Document;
use crate::workflow::common::StateEvent;

pub const TASK_RUN_KIND: &str = "workaholic/TaskRun:1.0";

pub type TaskRunDoc = Document<TaskRunSpec, TaskRunStatus>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskRunSpec {
    /// Reference to the TaskRunRequest that spawned this TaskRun.
    #[serde(rename = "taskRunRequestRef")]
    pub task_run_request_ref: String,
    /// Reference to the parent WorkRun.
    #[serde(rename = "workRunRef")]
    pub work_run_ref: String,
    /// Reference to the Work definition.
    #[serde(rename = "workRef")]
    pub work_ref: String,
    /// Reference to the Task definition.
    #[serde(rename = "taskRef")]
    pub task_ref: String,
    /// Name of the step within the Work this TaskRun corresponds to.
    #[serde(rename = "stepName")]
    pub step_name: String,
    /// Attempt number (1-based).
    pub attempt: u32,
    /// Reference to the WorkRunner executing this TaskRun.
    #[serde(rename = "workRunnerRef")]
    pub work_runner_ref: String,
    /// Reference to the TaskRunner handling this execution.
    #[serde(rename = "taskRunnerRef")]
    pub task_runner_ref: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskRunStatus {
    pub state: TaskRunState,
    #[serde(rename = "createdAt", default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    #[serde(rename = "startedAt", default, skip_serializing_if = "Option::is_none")]
    pub started_at: Option<String>,
    #[serde(rename = "finishedAt", default, skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<String>,
    /// Output values produced by the task (artifact URIs or scalar values).
    #[serde(default)]
    pub outputs: HashMap<String, Value>,
    #[serde(rename = "stateHistory", default)]
    pub state_history: Vec<StateEvent>,
    /// References to the captured stdout/stderr logs.
    #[serde(rename = "logsRef", default, skip_serializing_if = "Option::is_none")]
    pub logs_ref: Option<TaskRunLogsRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskRunLogsRef {
    pub stdout: String,
    pub stderr: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskRunState {
    #[default]
    Pending,
    Running,
    Succeeded,
    Failed,
    Cancelled,
}
