use serde::{Deserialize, Serialize};

use crate::document::Document;

pub type WorkerProfile = Document<WorkerProfileSpec>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerProfileSpec {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub scope: WorkerScope,
    #[serde(default)]
    pub concurrency: WorkerConcurrency,
    /// Allowed task runner kinds for workers using this profile.
    #[serde(default)]
    pub task_runners: Vec<String>,
    #[serde(default)]
    pub pools: Vec<WorkerPool>,
}

fn default_true() -> bool {
    true
}

impl Default for WorkerProfileSpec {
    fn default() -> Self {
        Self {
            enabled: true,
            scope: WorkerScope::default(),
            concurrency: WorkerConcurrency::default(),
            task_runners: Vec::new(),
            pools: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorkerScope {
    /// Namespace names this worker is allowed to process. Empty = all namespaces.
    #[serde(default)]
    pub namespaces: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerConcurrency {
    #[serde(default = "default_max_work_runs")]
    pub max_work_runs: usize,
    #[serde(default = "default_max_task_runs")]
    pub max_task_runs: usize,
}

fn default_max_work_runs() -> usize {
    4
}
fn default_max_task_runs() -> usize {
    16
}

impl Default for WorkerConcurrency {
    fn default() -> Self {
        Self {
            max_work_runs: default_max_work_runs(),
            max_task_runs: default_max_task_runs(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerPool {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<usize>,
}
