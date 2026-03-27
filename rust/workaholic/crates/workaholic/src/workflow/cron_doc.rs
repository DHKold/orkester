use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::document::Document;
use crate::utils::{default_true, default_utc};

pub const CRON_KIND: &str = "workaholic/Cron:1.0";

pub type CronDoc = Document<CronSpec, CronStatus>;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CronSpec {
    /// Whether this cron is active.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Work reference: `namespace/name:version` or `name:version`.
    pub work_ref: String,
    /// One or more cron expressions.
    #[serde(default)]
    pub schedules: Vec<String>,
    /// IANA timezone name (default: `UTC`).
    #[serde(default = "default_utc")]
    pub timezone: String,
    #[serde(default)]
    pub validity: ScheduleValidity,
    /// Static parameter overrides passed to each triggered WorkRun.
    #[serde(default)]
    pub params: Vec<CronParam>,
    #[serde(default)]
    pub concurrency: ConcurrencyPolicy,
    #[serde(default)]
    pub failure_policy: CronFailurePolicy,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CronStatus {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_scheduled_time: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_scheduled_time: Option<String>,
    /// Status of the last WorkRun spawned by this cron (e.g. `succeeded`, `failed`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_run_status: Option<String>,
    #[serde(default)]
    pub consecutive_failures: u32,
    #[serde(default)]
    pub run_count: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CronParam {
    pub name: String,
    /// Literal parameter value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<Value>,
    /// Artifact URI to use as parameter value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uri: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ScheduleValidity {
    /// ISO 8601 timestamp: earliest time a run may start.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start: Option<String>,
    /// ISO 8601 timestamp: latest time a run may start.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end: Option<String>,
    /// Maximum total runs within the validity window.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_runs: Option<u64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ConcurrencyPolicy {
    /// Behaviour when a new firing occurs while a previous run of the same cron is still executing.
    #[serde(default)]
    pub same_cron: ConcurrencyMode,
    /// Behaviour when another cron's run is still executing.
    #[serde(default)]
    pub different_cron: ConcurrencyMode,
    /// Behaviour when a manual run is still executing.
    #[serde(default)]
    pub manual: ConcurrencyMode,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ConcurrencyMode {
    #[default]
    Allow,
    Skip,
    Replace,
    Wait,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CronFailurePolicy {
    /// Maximum number of consecutive failures before taking action.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_consecutive_failures: Option<u32>,
    /// Action to take once the threshold is reached.
    #[serde(default)]
    pub action: FailurePolicyAction,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FailurePolicyAction {
    #[default]
    None,
    Disable,
}
