use serde::{Deserialize, Serialize};

use crate::document::Document;

pub const NAMESPACE_KIND: &str = "workaholic/Namespace:1.0";

pub type NamespaceDoc = Document<NamespaceSpec>;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NamespaceSpec {
    #[serde(default)]
    pub retention: RetentionPolicy,
    #[serde(default)]
    pub monthly_limits: NamespaceLimits,
    #[serde(default)]
    pub sizing_limits: NamespaceSizing,
}

/// Retention policy for a namespace, specifying how long to keep various types of data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetentionPolicy {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub work_runs_days: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_runs_days: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logs_days: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metrics_days: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifacts_days: Option<u32>,
}

impl Default for RetentionPolicy {
    fn default() -> Self {
        Self {
            work_runs_days: None,
            task_runs_days: None,
            logs_days: None,
            metrics_days: None,
            artifacts_days: None,
        }
    }
}

/// Consumption limits for a namespace, specifying maximum allowed usage on a given period.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NamespaceLimits {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_work_runs: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_task_runs: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_log_size_bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_artifacts_size_bytes: Option<u64>,
}

impl Default for NamespaceLimits {
    fn default() -> Self {
        Self {
            max_work_runs: None,
            max_task_runs: None,
            max_log_size_bytes: None,
            max_artifacts_size_bytes: None,
        }
    }
}

/// Sizing limits for a namespace, specifying maximum allowed concurrent usage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NamespaceSizing {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_workRunners: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_crons: Option<u32>,
}

impl Default for NamespaceSizing {
    fn default() -> Self {
        Self {
            max_workRunners: None,
            max_crons: None,
        }
    }
}
