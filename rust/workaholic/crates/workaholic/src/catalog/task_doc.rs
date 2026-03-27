use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::document::Document;
use crate::utils::{default_true, default_vec};

pub const TASK_KIND: &str = "workaholic/Task:1.0";

pub type TaskDoc = Document<TaskSpec>;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TaskSpec {
    /// List of task inputs. These define the parameters that must be provided when executing the task, either by a parent Work or by a TaskRunner.
    #[serde(default = "default_vec")]
    pub inputs: Vec<TaskInput>,
    /// List of task outputs. These define the values produced by the task execution that can be consumed by a parent Work or a downstream task.
    #[serde(default = "default_vec")]
    pub outputs: Vec<TaskOutput>,
    /// Execution specification defining how to run the task, including the runner kind, optional profile reference, and runner-specific configuration.
    pub execution: ExecutionSpec,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskInput {
    /// Name of the parameter, used for referencing it in input mappings and work execution.
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Optional description of the parameter.
    pub description: Option<String>,
    /// Semantic type of the parameter (e.g. `string`, `file`, `integer`, `boolean`, etc.).
    #[serde(rename = "type")]
    pub param_type: String,
    /// Whether this parameter is required or optional (default: true).
    #[serde(default = "default_true")]
    pub required: bool,
    /// Default value for the parameter, used when the parameter is not provided at runtime. Can be a literal value or an artifact reference.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<TaskInputSource>,
}

/// Input Source, either a reference to an artifact or a literal value.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum TaskInputSource {
    /// Literal value: `default: { value: "foo" }`
    Literal { value: Value },
    /// Artifact reference: `default: { uri: "registry://..." }`
    ArtifactRef { uri: String },
}

/// Output description for a task output, used for referencing it in output mappings and work execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskOutput {
    /// Name of the output, used for referencing it in output mappings and work execution.
    pub name: String,
    /// Optional description of the output.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Semantic type of the output (e.g. `string`, `file`, `integer`, `boolean`, etc.).
    #[serde(rename = "type")]
    pub output_type: String,
}

/// Execution specification defining how to run the task, including the runner kind, default profile reference, and runner-specific configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExecutionSpec {
    /// Component kind for this runner, e.g. `workaholic/shell-runner:1.0`.
    pub kind: String,
    /// Optional reference to a TaskRunnerProfile by `namespace/name:version`.
    pub profile: String,
    /// Runner-specific configuration (script, env vars, file mounts, etc.).
    #[serde(default)]
    pub config: Value,
}
