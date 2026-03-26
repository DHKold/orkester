use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::document::Document;
use crate::workflow::common::StateEvent;

pub const WORKER_KIND: &str = "workaholic/Worker:1.0";

pub type Worker = Document<WorkerSpec, WorkerStatus>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerSpec {
    /// Component kind for this worker, e.g. `workaholic/ThreadWorker:1.0`.
    pub kind: String,
    /// Kind-specific configuration (thread count, etc.).
    #[serde(default)]
    pub config: Value,
    /// Runtime concurrency limits (active runs allowed simultaneously).
    #[serde(default)]
    pub concurrency: WorkerConcurrencyLimits,
    /// Quota caps applied over a rolling period.
    #[serde(default)]
    pub quotas: WorkerQuotas,
    /// Restrictions on which task runner kinds are permitted on this worker.
    #[serde(default)]
    pub restrictions: WorkerRestrictions,
    /// Arbitrary key/value labels for routing and discovery.
    #[serde(default)]
    pub labels: Map<String, Value>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorkerConcurrencyLimits {
    /// Maximum number of concurrently active WorkRuns.
    #[serde(default)]
    pub max_work_runs: usize,
    /// Maximum number of concurrently active TaskRuns (across all WorkRuns).
    #[serde(default)]
    pub max_task_runs: usize,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorkerQuotas {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_work_runs: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_task_runs: Option<u64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorkerRestrictions {
    /// Allowed task runner kinds (e.g. `shell`, `container`, `kubernetes`, `sql`).
    #[serde(rename = "taskRunners", default)]
    pub task_runners: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerStatus {
    pub state: WorkerState,
    #[serde(default)]
    pub active_work_runs: usize,
    #[serde(default)]
    pub active_task_runs: usize,
    #[serde(rename = "stateHistory", default)]
    pub state_history: Vec<StateEvent>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WorkerState {
    #[default]
    Creating,
    Active,
    Inactive,
    Dropped,
}
