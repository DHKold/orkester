//! Workflow server configuration.

use serde::Deserialize;
use workaholic::WorkRunnerSpec;

/// Configuration for the `WorkflowServerComponent`.
#[derive(Debug, Clone, Deserialize)]
pub struct WorkflowServerConfig {
    /// Name of this server instance.
    pub name: String,
    /// Namespace this server operates in.
    pub namespace: String,
    /// Concurrency limits for the work runner.
    pub work_runner: WorkRunnerSpec,
    /// How many task permits to grant per work run scheduling cycle.
    #[serde(default = "default_task_permits")]
    pub task_permits_per_grant: usize,
    /// Reference to the catalog server component (for resolving Work/Task docs).
    #[serde(default)]
    pub catalog_ref: String,
}

fn default_task_permits() -> usize { 4 }
