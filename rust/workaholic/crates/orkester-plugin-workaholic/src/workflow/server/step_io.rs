//! Helpers for collecting step outputs and resolving step-output input references.

use std::collections::HashMap;

use serde_json::Value;
use workaholic::{TaskInputSource, TaskRunDoc, TaskRunRequestDoc};

use crate::workflow::trigger::input_resolver::parse_step_output_ref;

/// Collect all named outputs from a completed `TaskRunDoc`.
pub fn collect_step_outputs(doc: &TaskRunDoc) -> HashMap<String, Value> {
    doc.status.as_ref()
        .map(|s| s.outputs.clone())
        .unwrap_or_default()
}

/// Resolve `work://steps/<step>/outputs?<name>` inputs using completed step outputs.
pub fn resolve_step_inputs(
    mut req: TaskRunRequestDoc,
    step_outputs: &HashMap<String, HashMap<String, Value>>,
) -> TaskRunRequestDoc {
    for input in &mut req.spec.inputs {
        let uri = match &input.from {
            TaskInputSource::ArtifactRef { uri } => uri.clone(),
            _ => continue,
        };
        if let Some((step, name)) = parse_step_output_ref(&uri) {
            if let Some(val) = step_outputs.get(step).and_then(|m| m.get(name)) {
                input.from = TaskInputSource::Literal { value: val.clone() };
            }
        }
    }
    req
}
