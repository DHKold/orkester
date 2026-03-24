use serde::{Deserialize, Serialize};

use crate::document::Document;

pub type TaskRunnerProfile = Document<TaskRunnerProfileSpec>;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TaskRunnerProfileSpec {
    /// Task runner kind this profile applies to (shell, container, kubernetes, …).
    pub kind: String,
    #[serde(default)]
    pub config: serde_json::Value,
    /// Whether individual tasks can override fields in this profile.
    #[serde(default)]
    pub allow_override: bool,
    #[serde(default)]
    pub allowed_override_fields: Vec<String>,
}
