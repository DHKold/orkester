use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::document::Document;

pub const TASK_RUNNER_PROFILE_KIND: &str = "workaholic/TaskRunnerProfile:1.0";

pub type TaskRunnerProfileDoc = Document<TaskRunnerProfileSpec>;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TaskRunnerProfileSpec {
    #[serde(flatten)]
    pub properties: Map<String, Value>,
}
