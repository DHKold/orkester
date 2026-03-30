use serde::{Deserialize, Serialize};
use serde_json::Value;
use workaholic::{CronDoc, TaskRunDoc, WorkRunDoc};

// ─── TaskRunner requests ──────────────────────────────────────────────────────

/// Used by `ACTION_TASK_RUNNER_SPAWN`.
pub type SpawnTaskRunRequest = workaholic::TaskRunRequestDoc;

// ─── WorkRunner requests ──────────────────────────────────────────────────────

/// Used by `ACTION_WORK_RUNNER_SPAWN`.
pub type SpawnWorkRunRequest = workaholic::WorkRunRequestDoc;

// ─── Workflow server requests ─────────────────────────────────────────────────

/// Manual-trigger a Work execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriggerWorkRequest {
    /// Fully-qualified work reference: `namespace/name:version`.
    #[serde(rename = "workRef")]
    pub work_ref: String,
    /// Optional static input overrides.
    #[serde(default)]
    pub inputs: std::collections::HashMap<String, Value>,
}

/// Reference to a WorkRun by its name.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkRunRefRequest {
    pub name: String,
}

/// Reference to a TaskRun by its name.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskRunRefRequest {
    pub name: String,
}

/// Reference to a Cron by its name.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CronRefRequest {
    pub name: String,
}

// ─── Workflow server responses ────────────────────────────────────────────────

/// Response to `ACTION_WORKFLOW_LIST_WORK_RUNS`.
#[derive(Debug, Clone, Serialize)]
pub struct ListWorkRunsResponse {
    pub work_runs: Vec<WorkRunDoc>,
}

/// Response to `ACTION_WORKFLOW_LIST_TASK_RUNS`.
#[derive(Debug, Clone, Serialize)]
pub struct ListTaskRunsResponse {
    pub task_runs: Vec<TaskRunDoc>,
}

/// Response to `ACTION_WORKFLOW_LIST_CRONS`.
#[derive(Debug, Clone, Serialize)]
pub struct ListCronsResponse {
    pub crons: Vec<CronDoc>,
}
