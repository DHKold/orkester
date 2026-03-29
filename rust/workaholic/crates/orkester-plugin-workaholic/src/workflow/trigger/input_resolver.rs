//! Resolves task input values from `WorkInputSource` / `TaskInputSource`.
//!
//! At resolution time every input in a `WorkStep` has a `from` describing its
//! origin.  This module produces a concrete [`ResolvedInput`] for each one.

use serde_json::Value;
use workaholic::{
    ResolvedInput, ResolvedOutput, StepInputMapping, StepOutputMapping, TaskInput,
    TaskInputSource, WorkInputSource, WorkOutputSource,
};

// ─── Input ────────────────────────────────────────────────────────────────────

/// Resolve one step input mapping into a `ResolvedInput`.
///
/// `work_inputs` is the map of work-level input values resolved from the
/// trigger (name → concrete value).
pub fn resolve_input(
    task_input:   &TaskInput,
    mapping:      Option<&StepInputMapping>,
    work_inputs:  &std::collections::HashMap<String, Value>,
) -> ResolvedInput {
    let from = mapping
        .map(|m| resolve_source_from_work(&m.from, work_inputs))
        .or_else(|| task_input.default.as_ref().map(|d| d.clone()))
        .unwrap_or(TaskInputSource::Literal { value: Value::Null });

    ResolvedInput {
        name:        task_input.name.clone(),
        description: task_input.description.clone(),
        input_type:  Some(task_input.param_type.clone()),
        required:    task_input.required,
        from,
    }
}

/// Map a `WorkInputSource` to a `TaskInputSource`.
fn resolve_source_from_work(
    src:         &WorkInputSource,
    work_inputs: &std::collections::HashMap<String, Value>,
) -> TaskInputSource {
    match src {
        WorkInputSource::Literal { value } => TaskInputSource::Literal { value: value.clone() },
        WorkInputSource::ArtifactRef { uri } => {
            // Parse "work://inputs?<name>" to extract the bare input name, then
            // substitute with the resolved work-level value if present.
            let key = parse_work_input_name(uri).unwrap_or(uri.as_str());
            if let Some(v) = work_inputs.get(key) {
                TaskInputSource::Literal { value: v.clone() }
            } else {
                TaskInputSource::ArtifactRef { uri: uri.clone() }
            }
        }
    }
}

/// Extract the input name from a `work://inputs?<name>` URI.
fn parse_work_input_name(uri: &str) -> Option<&str> {
    uri.strip_prefix("work://inputs?")
}

// ─── Output ───────────────────────────────────────────────────────────────────

/// Resolve one output mapping into a `ResolvedOutput`.
pub fn resolve_output(
    output_name: &str,
    mapping:     Option<&StepOutputMapping>,
) -> ResolvedOutput {
    let to = mapping
        .map(|m| m.to.clone())
        .unwrap_or(WorkOutputSource::Variable { variable: output_name.to_string() });
    ResolvedOutput {
        name:        output_name.to_string(),
        description: None,
        output_type: None,
        to,
    }
}
