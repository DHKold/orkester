use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::document::Document;
use crate::workflow::common::StateEvent;

pub const TASK_RUNNER_KIND: &str = "workaholic/TaskRunner:1.0";

pub type TaskRunner = Document<TaskRunnerSpec, TaskRunnerStatus>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskRunnerSpec {
    /// Component kind for this task runner, e.g. `workaholic/ShellTaskRunner:1.0`.
    pub kind: String,
    /// Kind-specific configuration (timeout, user, resource limits, etc.).
    #[serde(default)]
    pub config: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskRunnerStatus {
    pub state: TaskRunnerState,
    #[serde(default)]
    pub metrics: TaskRunnerMetrics,
    #[serde(rename = "stateHistory", default)]
    pub state_history: Vec<StateEvent>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TaskRunnerMetrics {
    /// Total wall-clock seconds the runner spent executing tasks.
    #[serde(default)]
    pub total_time_seconds: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskRunnerState {
    #[default]
    Creating,
    Ready,
    Running,
    Dropped,
}
