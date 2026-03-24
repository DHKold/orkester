use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::document::Document;

pub type WorkRun = Document<WorkRunSpec, WorkRunStatus>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkRunSpec {
    /// Reference to the Work: `namespace/name` or `namespace/name:version`.
    pub work_ref: String,
    pub trigger: WorkRunTrigger,
    #[serde(default)]
    pub params: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkRunTrigger {
    pub kind: TriggerKind,
    /// Cron name, API caller ID, or other reference.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TriggerKind {
    #[default]
    Manual,
    Cron,
    Api,
    System,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorkRunStatus {
    pub phase: WorkRunPhase,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_at: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub task_counts: TaskCounts,
    /// Worker currently executing this run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worker: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum WorkRunPhase {
    #[default]
    Pending,
    Ready,
    Running,
    Succeeded,
    Failed,
    Cancelled,
}

impl WorkRunPhase {
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Succeeded | Self::Failed | Self::Cancelled)
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TaskCounts {
    #[serde(default)]
    pub total: u32,
    #[serde(default)]
    pub pending: u32,
    #[serde(default)]
    pub running: u32,
    #[serde(default)]
    pub succeeded: u32,
    #[serde(default)]
    pub failed: u32,
    #[serde(default)]
    pub cancelled: u32,
    #[serde(default)]
    pub skipped: u32,
}
