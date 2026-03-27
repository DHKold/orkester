use serde::{Deserialize, Serialize};

use crate::document::Document;

pub const WORKER_PROFILE_KIND: &str = "workaholic/WorkRunnerProfile:1.0";

pub type WorkRunnerProfileDoc = Document<WorkRunnerProfileSpec>;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorkRunnerProfileSpec {
    #[serde(default)]
    pub concurrency: WorkRunnerConcurrency,
    /// Allowed runner kinds (e.g. `orkester/ShellRunner:1.0`, `orkester/ContainerRunner:1.0`, `orkester/KubernetesRunner:1.0`, `orkester/SQLRunner:1.0`).
    #[serde(rename = "runnerWhitelist", default)]
    pub runner_whitelist: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkRunnerConcurrency {
    // Maximum number of concurrent WorkRuns on the WorkRunner.
    #[serde(default = "default_max_work_runs")]
    pub max_work_runs: usize,
    // Maximum number of concurrent TaskRuns on the WorkRunner (across all WorkRuns).
    #[serde(default = "default_max_task_runs")]
    pub max_task_runs: usize,
}

fn default_max_work_runs() -> usize { 4 }
fn default_max_task_runs() -> usize { 16 }

impl Default for WorkRunnerConcurrency {
    fn default() -> Self {
        Self {
            max_work_runs: default_max_work_runs(),
            max_task_runs: default_max_task_runs(),
        }
    }
}
