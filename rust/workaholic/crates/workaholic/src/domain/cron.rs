use serde::{Deserialize, Serialize};

use crate::document::Document;

pub type Cron = Document<CronSpec>;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CronSpec {
    /// Work reference: `namespace/name:version` or `name`.
    pub work_ref: String,
    /// One or more cron expressions.
    #[serde(default)]
    pub schedules: Vec<String>,
    #[serde(default = "default_utc")]
    pub timezone: String,
    #[serde(default)]
    pub validity: ScheduleValidity,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_runs: Option<u64>,
    #[serde(default)]
    pub params: serde_json::Value,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub concurrency: ConcurrencyPolicy,
    #[serde(default)]
    pub failure_policy: CronFailurePolicy,
}

fn default_utc() -> String {
    "UTC".to_string()
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ScheduleValidity {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_at: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ConcurrencyPolicy {
    #[serde(default)]
    pub same_cron: ConcurrencyMode,
    #[serde(default)]
    pub other_crons: ConcurrencyMode,
    #[serde(default)]
    pub manual: ConcurrencyMode,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ConcurrencyMode {
    #[default]
    Skip,
    Replace,
    Add,
    Wait,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CronFailurePolicy {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_consecutive_failures: Option<u32>,
    #[serde(default)]
    pub action: FailurePolicyAction,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FailurePolicyAction {
    #[default]
    None,
    Pause,
    Disable,
}
