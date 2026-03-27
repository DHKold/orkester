use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::document::Document;
use crate::utils::default_true;

pub const WORK_KIND: &str = "workaholic/Work:1.0";

pub type WorkDoc = Document<WorkSpec>;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorkSpec {
    #[serde(default)]
    pub inputs: Vec<WorkInput>,
    #[serde(default)]
    pub steps: Vec<WorkStep>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkInput {
    /// Name of the input, used for referencing it in step input mappings.
    pub name: String,
    /// Optional description of the input.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Semantic type of the input (e.g. `string`, `file`, `integer`, `boolean`, etc.).
    #[serde(rename = "type")]
    pub input_type: String,
    /// Whether this input is required or optional (default: true).
    #[serde(default="default_true")]
    pub required: bool,
    /// Whether this input is editable (default: true).
    #[serde(default="default_true")]
    pub editable: bool,
    /// Optional default value for the input, used when the input is not provided at runtime.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<WorkInputSource>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkStep {
    /// Node name — unique within the Work definition.
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Reference to a Task document: `name:version`, `namespace/name:version`, etc.
    #[serde(rename = "taskRef")]
    pub task_ref: String,
    /// Names of steps this one depends on (DAG edges).
    #[serde(rename = "dependsOn", default)]
    pub depends_on: Vec<String>,
    /// Input mappings: how each task input is sourced.
    #[serde(rename = "inputMapping", default)]
    pub input_mapping: Vec<StepInputMapping>,
    /// Output mappings: what to do with each task output.
    #[serde(rename = "outputMapping", default)]
    pub output_mapping: Vec<StepOutputMapping>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepInputMapping {
    /// Name of the task input this mapping applies to.
    pub name: String,
    /// Source of the input value: either an artifact/work input reference or a literal value.
    pub from: WorkInputSource,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepOutputMapping {
    /// Name of the task output this mapping applies to.
    pub name: String,
    /// Destination for the output value: either an artifact reference or a work output reference.
    pub to: WorkOutputSource,
}

/// Input Source for a task input, either a reference to an artifact or a literal value.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum WorkInputSource {
    /// Literal value: `default: { value: "foo" }`
    Literal { value: Value },
    /// Artifact reference: `default: { uri: "registry://..." }`
    ArtifactRef { uri: String },
}

/// Output destination for a task output, either a reference to an artifact or a work output.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum WorkOutputSource {
    /// Artifact reference with optional retention (e.g. `"30d"`, `"ephemeral"`).
    ArtifactRef {
        uri: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        retention: Option<String>,
    },
    /// Variable reference: `to: { variable: "output_name" }`
    Variable { variable: String },
}
