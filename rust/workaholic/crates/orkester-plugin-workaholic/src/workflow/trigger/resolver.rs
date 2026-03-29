//! `TriggerResolver` — converts a lightweight `Trigger` into fully-frozen
//! execution requests.
//!
//! The resolver:
//!
//! 1. Accepts a `WorkDoc` (already loaded from the catalog) and its `TaskDoc`s.
//! 2. Merges trigger-level input overrides with Work-level defaults.
//! 3. For every `WorkStep`, resolves all inputs and outputs into a
//!    `TaskRunRequestDoc`.
//! 4. Returns one `WorkRunRequestDoc` (with step references) plus a vec of
//!    `TaskRunRequestDoc`s.

use std::collections::HashMap;

use serde_json::Value;
use uuid::Uuid;
use workaholic::{
    DocumentMetadata, ResolvedInput, TaskDoc, TaskInputSource, Trigger, WorkDoc,
    WorkRunRequestDoc, WorkRunRequestSpec, WorkRunRequestStep,
    TaskRunRequestDoc, TaskRunRequestSpec,
    TASK_RUN_REQUEST_KIND, WORK_RUN_REQUEST_KIND,
};

use super::input_resolver::{resolve_input, resolve_output};

// ─── TriggerResolver ─────────────────────────────────────────────────────────

/// Converts a `Trigger` plus the referenced catalog documents into frozen
/// `WorkRunRequest` and `TaskRunRequest` documents.
pub struct TriggerResolver;

/// Input provided to `TriggerResolver::resolve`.
pub struct ResolutionInput<'a> {
    /// The Work definition from the catalog.
    pub work:          &'a WorkDoc,
    /// Task definitions keyed by task ref (`namespace/name:version`).
    pub tasks:         &'a HashMap<String, TaskDoc>,
    /// The trigger that fired (manual, cron, etc.).
    pub trigger:       Trigger,
    /// Trigger-level input overrides (name → literal value).
    pub input_overrides: HashMap<String, Value>,
    /// Reference string for the WorkRunner that will execute this run.
    pub work_runner_ref: String,
}

/// Output produced by `TriggerResolver::resolve`.
pub struct ResolutionOutput {
    pub work_run_request:  WorkRunRequestDoc,
    pub task_run_requests: Vec<TaskRunRequestDoc>,
}

impl TriggerResolver {
    /// Resolve a trigger into a frozen set of execution requests.
    pub fn resolve(input: ResolutionInput<'_>) -> ResolutionOutput {
        let request_name = Uuid::new_v4().to_string();

        let work_inputs = build_work_inputs(input.work, &input.input_overrides);
        let (steps, task_requests) = resolve_steps(
            input.work,
            input.tasks,
            &work_inputs,
            &request_name,
            &input.work_runner_ref,
        );

        let work_run_request = WorkRunRequestDoc {
            kind:    WORK_RUN_REQUEST_KIND.to_string(),
            name:    request_name,
            version: "1.0.0".to_string(),
            metadata: DocumentMetadata {
                namespace: input.work.metadata.namespace.clone(),
                owner: None, description: None, tags: vec![], extra: Default::default(),
            },
            spec: WorkRunRequestSpec {
                work_ref: work_ref_from(input.work),
                trigger:  input.trigger,
                inputs:   work_inputs_to_source_map(&work_inputs),
                steps,
            },
            status: None,
        };

        ResolutionOutput { work_run_request, task_run_requests: task_requests }
    }
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

/// Merge Work-level input defaults with trigger overrides.
fn build_work_inputs(work: &WorkDoc, overrides: &HashMap<String, Value>) -> HashMap<String, Value> {
    let mut map: HashMap<String, Value> = HashMap::new();
    for input in &work.spec.inputs {
        if let Some(default) = &input.default {
            let val = match default {
                workaholic::WorkInputSource::Literal { value } => value.clone(),
                workaholic::WorkInputSource::ArtifactRef { uri } => Value::String(uri.clone()),
            };
            map.insert(input.name.clone(), val);
        }
    }
    for (k, v) in overrides {
        map.insert(k.clone(), v.clone());
    }
    map
}

/// Convert the work-level resolved value map to a `TaskInputSource` map.
fn work_inputs_to_source_map(
    inputs: &HashMap<String, Value>,
) -> HashMap<String, TaskInputSource> {
    inputs.iter()
        .map(|(k, v)| (k.clone(), TaskInputSource::Literal { value: v.clone() }))
        .collect()
}

/// Build the canonical work ref string from a `WorkDoc`.
fn work_ref_from(work: &WorkDoc) -> String {
    match &work.metadata.namespace {
        Some(ns) => format!("{}/{}", ns, work.name),
        None     => work.name.clone(),
    }
}

/// Resolve all steps, building both `WorkRunRequestStep` and `TaskRunRequestDoc` lists.
fn resolve_steps(
    work:              &WorkDoc,
    tasks:             &HashMap<String, TaskDoc>,
    work_inputs:       &HashMap<String, Value>,
    work_request_ref:  &str,
    work_runner_ref:   &str,
) -> (Vec<WorkRunRequestStep>, Vec<TaskRunRequestDoc>) {
    let mut steps:         Vec<WorkRunRequestStep>  = Vec::new();
    let mut task_requests: Vec<TaskRunRequestDoc>   = Vec::new();

    for step in &work.spec.steps {
        let task = tasks.get(&step.task_ref);
        let (req_doc, req_ref) = build_task_run_request(
            step, task, work_inputs, work_request_ref, work_runner_ref, work,
        );
        steps.push(WorkRunRequestStep {
            name:                step.name.clone(),
            description:         step.description.clone(),
            depends_on:          step.depends_on.clone(),
            task_run_request_ref: req_ref,
        });
        task_requests.push(req_doc);
    }

    (steps, task_requests)
}

/// Build a `TaskRunRequestDoc` for one step.
fn build_task_run_request(
    step:             &workaholic::WorkStep,
    task:             Option<&TaskDoc>,
    work_inputs:      &HashMap<String, Value>,
    work_request_ref: &str,
    _work_runner_ref: &str,
    work:             &WorkDoc,
) -> (TaskRunRequestDoc, String) {
    let req_name = Uuid::new_v4().to_string();

    let inputs  = resolve_task_inputs(step, task, work_inputs);
    let outputs = resolve_task_outputs(step, task);
    let execution = task.map(|t| t.spec.execution.clone())
        .unwrap_or_default();

    let doc = TaskRunRequestDoc {
        kind:    TASK_RUN_REQUEST_KIND.to_string(),
        name:    req_name.clone(),
        version: "1.0.0".to_string(),
        metadata: DocumentMetadata {
            namespace: work.metadata.namespace.clone(),
            owner: None, description: None, tags: vec![], extra: Default::default(),
        },
        spec: TaskRunRequestSpec {
            work_ref:            work_ref_from(work),
            task_ref:            step.task_ref.clone(),
            work_run_request_ref: work_request_ref.to_string(),
            step_name:           step.name.clone(),
            inputs,
            outputs,
            execution,
        },
        status: None,
    };

    (doc, req_name)
}

/// Resolve task inputs by matching step input mappings to task input definitions.
fn resolve_task_inputs(
    step:        &workaholic::WorkStep,
    task:        Option<&TaskDoc>,
    work_inputs: &HashMap<String, Value>,
) -> Vec<ResolvedInput> {
    let empty = Vec::new();
    let task_inputs = task.map(|t| &t.spec.inputs).unwrap_or(&empty);
    task_inputs.iter().map(|ti| {
        let mapping = step.input_mapping.iter().find(|m| m.name == ti.name);
        resolve_input(ti, mapping, work_inputs)
    }).collect()
}

/// Resolve task outputs by matching step output mappings.
fn resolve_task_outputs(
    step: &workaholic::WorkStep,
    task: Option<&TaskDoc>,
) -> Vec<workaholic::ResolvedOutput> {
    let empty = Vec::new();
    let task_outputs = task.map(|t| &t.spec.outputs).unwrap_or(&empty);
    task_outputs.iter().map(|to| {
        let mapping = step.output_mapping.iter().find(|m| m.name == to.name);
        resolve_output(&to.name, mapping)
    }).collect()
}
