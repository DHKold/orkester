use serde::{Deserialize, Serialize};

use crate::document::Document;

pub type Namespace = Document<NamespaceSpec>;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NamespaceSpec {
    #[serde(default)]
    pub retention: RetentionPolicy,
}

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
            work_runs_days: Some(30),
            task_runs_days: Some(30),
            logs_days: Some(7),
            metrics_days: Some(90),
            artifacts_days: Some(30),
        }
    }
}
