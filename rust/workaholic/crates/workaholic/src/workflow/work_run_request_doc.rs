use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::catalog::TaskInputSource;
use crate::document::Document;
use crate::workflow::common::Trigger;

pub const WORK_RUN_REQUEST_KIND: &str = "workaholic/WorkRunRequest:1.0";

pub type WorkRunRequestDoc = Document<WorkRunRequestSpec>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkRunRequestSpec {
    /// Reference to the Work definition: `namespace/name:version`.
    #[serde(rename = "workRef")]
    pub work_ref: String,
    /// What caused this run to be requested.
    pub trigger: Trigger,
    /// Resolved input values indexed by input name (literal or artifact reference).
    #[serde(default)]
    pub inputs: HashMap<String, TaskInputSource>,
    /// Resolved step requests in DAG order.
    #[serde(default)]
    pub steps: Vec<WorkRunRequestStep>,
}

/// A resolved step inside a WorkRunRequest, pointing to its TaskRunRequest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkRunRequestStep {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Names of steps this one depends on.
    #[serde(rename = "dependsOn", default)]
    pub depends_on: Vec<String>,
    /// Reference to the corresponding TaskRunRequest document.
    #[serde(rename = "taskRunRequestRef")]
    pub task_run_request_ref: String,
    /// Maximum number of attempts before the step is permanently failed (default: 1 = no retry).
    #[serde(rename = "maxAttempts", default = "default_max_attempts")]
    pub max_attempts: u32,
    /// Delay in seconds between retry attempts (default: 0).
    #[serde(rename = "retryDelaySecs", default)]
    pub retry_delay_secs: u64,
}

fn default_max_attempts() -> u32 { 1 }
