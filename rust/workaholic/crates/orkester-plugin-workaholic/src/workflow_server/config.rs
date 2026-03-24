use serde::{Deserialize, Serialize};

use crate::worker::WorkerConfig;

/// Configuration passed to the WorkflowServer at creation time.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorkflowServerConfig {
    /// Persistence backend configuration.
    #[serde(default)]
    pub persistence: PersistenceConfig,
    /// Workers to create at startup.
    #[serde(default)]
    pub workers: Vec<WorkerConfig>,
    /// Namespace used when namespace is not provided in requests.
    #[serde(default = "default_namespace")]
    pub default_namespace: String,
}

fn default_namespace() -> String {
    "default".to_string()
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PersistenceConfig {
    #[default]
    Memory,
    LocalFs {
        path: String,
    },
}
