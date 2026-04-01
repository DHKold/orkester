use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

use crate::document::Document;
use crate::workflow::common::{StateEvent, Trigger};

pub const WORK_RUN_KIND: &str = "workaholic/WorkRun:1.0";

pub type WorkRunDoc = Document<WorkRunSpec, WorkRunStatus>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkRunSpec {
    /// Reference to the WorkRunRequest that spawned this run.
    #[serde(rename = "workRunRequestRef")]
    pub work_run_request_ref: String,
    /// Reference to the Work definition being executed.
    #[serde(rename = "workRef")]
    pub work_ref: String,
    /// Reference to the WorkRunner executing this run.
    #[serde(rename = "workRunnerRef")]
    pub work_runner_ref: String,
    /// What caused this run to start.
    pub trigger: Trigger,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorkRunStatus {
    pub state: WorkRunState,
    #[serde(rename = "createdAt", default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    #[serde(rename = "startedAt", default, skip_serializing_if = "Option::is_none")]
    pub started_at: Option<String>,
    #[serde(rename = "finishedAt", default, skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<String>,
    #[serde(default)]
    pub summary: WorkRunSummary,
    /// Per-step runtime state.
    #[serde(default)]
    pub steps: Vec<WorkRunStepStatus>,
    /// Output values produced by the WorkRun (artifact URIs or scalar values).
    #[serde(default)]
    pub outputs: HashMap<String, Value>,
    #[serde(rename = "stateHistory", default)]
    pub state_history: Vec<StateEvent>,
    /// Structured log entries produced during execution.
    #[serde(default)]
    pub logs: Vec<WorkRunLogEntry>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorkRunSummary {
    #[serde(rename = "totalSteps", default)]
    pub total_steps: usize,
    #[serde(rename = "pendingSteps", default)]
    pub pending_steps: usize,
    #[serde(rename = "runningSteps", default)]
    pub running_steps: usize,
    #[serde(rename = "succeededSteps", default)]
    pub succeeded_steps: usize,
    #[serde(rename = "failedSteps", default)]
    pub failed_steps: usize,
    #[serde(rename = "cancelledSteps", default)]
    pub cancelled_steps: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkRunStepStatus {
    pub name: String,
    pub state: WorkRunState,
    #[serde(rename = "taskRunRequestRef", default, skip_serializing_if = "Option::is_none")]
    pub task_run_request_ref: Option<String>,
    #[serde(rename = "activeTaskRunRef", default, skip_serializing_if = "Option::is_none")]
    pub active_task_run_ref: Option<String>,
    #[serde(default)]
    pub attempts: u32,
}

/// A single structured log entry emitted during a WorkRun execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkRunLogEntry {
    /// ISO-8601 timestamp.
    pub ts:      String,
    /// Severity: "info" | "warn" | "error".
    pub level:   String,
    /// Human-readable message.
    pub message: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WorkRunState {
    #[default]
    Pending,
    Running,
    Succeeded,
    Failed,
    Cancelled,
}