use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::document::Document;

pub type TaskRun = Document<TaskRunSpec, TaskRunStatus>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskRunSpec {
    /// ID (name) of the owning WorkRun.
    pub work_run_ref: String,
    /// Name of the task node inside the Work definition.
    pub task_name: String,
    /// Reference to the Task document: `name` or `name:version`.
    pub task_ref: String,
    /// Attempt number, starting at 1. Increments on each retry.
    #[serde(default = "default_one")]
    pub attempt: u32,
    /// Resolved inputs for this execution attempt.
    #[serde(default)]
    pub inputs: serde_json::Value,
}

fn default_one() -> u32 {
    1
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TaskRunStatus {
    pub phase: TaskRunPhase,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_at: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<DateTime<Utc>>,
    /// Worker that started or is running this task.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worker: Option<String>,
    /// Task runner instance that executed or is executing this task.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_runner: Option<String>,
    /// External execution ID (e.g. Kubernetes job name, container ID).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external_id: Option<String>,
    /// Output artifacts/values produced by the task.
    #[serde(default)]
    pub outputs: serde_json::Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<TaskRunError>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum TaskRunPhase {
    #[default]
    Pending,
    Ready,
    Starting,
    Running,
    Succeeded,
    Failed,
    Cancelled,
    Skipped,
}

impl TaskRunPhase {
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            Self::Succeeded | Self::Failed | Self::Cancelled | Self::Skipped
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskRunError {
    #[serde(default)]
    pub code: String,
    pub message: String,
}
