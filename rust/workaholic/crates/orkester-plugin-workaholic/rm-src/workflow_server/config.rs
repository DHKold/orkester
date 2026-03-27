use serde::{Deserialize, Serialize};

use crate::workRunner::WorkRunnerConfig;

/// Configuration passed to the WorkflowServer at creation time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowServerConfig {
    /// Registered name of the persistence component to use
    /// (e.g. `"local-fs-persistence"` or `"memory-persistence"`).
    ///
    /// The routing host forwards `persistence/*` actions to the component
    /// whose registered name contains `"persistence"`.
    #[serde(default = "default_persistence")]
    pub persistence: String,

    /// WorkRunners to spawn at startup (inline threads managed by this server).
    #[serde(default)]
    pub workRunners: Vec<WorkRunnerConfig>,

    /// Namespace used when namespace is not provided in requests.
    #[serde(default = "default_namespace")]
    pub default_namespace: String,
}

fn default_persistence() -> String {
    "memory-persistence".to_string()
}

fn default_namespace() -> String {
    "default".to_string()
}

impl Default for WorkflowServerConfig {
    fn default() -> Self {
        Self {
            persistence:       default_persistence(),
            workRunners:           Vec::new(),
            default_namespace: default_namespace(),
        }
    }
}
