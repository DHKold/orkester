//! Direct task executor — runs steps in topological order using ad-hoc runners.
//!
//! Bypasses the hub dispatch system to avoid pipeline re-entrancy deadlocks.

use std::collections::{HashMap, HashSet};
use std::time::Duration;

use serde_json::Value;
use workaholic::{
    TaskRunDoc, TaskRunRequestDoc, TaskRunnerSpec, TaskRunState,
    WorkRunRequestDoc, WorkRunState,
};

use super::registry::WorkflowRegistry;
use super::run_log::append_run_log;
use super::step_io::{collect_step_outputs, resolve_step_inputs};
use crate::workflow::task_runner::{
    ContainerTaskRunner, HttpTaskRunner, KubernetesTaskRunner, ShellTaskRunner,
    TaskRunner,
};

// ─── Public entry point ───────────────────────────────────────────────────────

/// Run all steps in `request` in topological order, updating the registry.
pub fn execute_work_run(
    request:       &WorkRunRequestDoc,
    task_requests: &HashMap<String, TaskRunRequestDoc>,
    registry:      &WorkflowRegistry,
) {
    let run_name         = &request.name;
    let mut completed:    HashSet<String>                      = HashSet::new();
    let mut step_outputs: HashMap<String, HashMap<String, Value>> = HashMap::new();
    let mut remaining:   Vec<_>          = request.spec.steps.iter().collect();
    let mut all_ok                       = true;

    while !remaining.is_empty() {
        // Find the first step whose every dependency has completed.
        let idx = remaining.iter().position(|s| {
            s.depends_on.iter().all(|d| completed.contains(d))
        });
        let step = match idx {
            Some(i) => remaining.remove(i),
            None => {
                eprintln!("[executor] {run_name}: no ready step — possible cycle");
                append_run_log(registry, run_name, "error", "No ready step — possible dependency cycle".into());
                all_ok = false;
                break;
            }
        };

        let req = match task_requests.get(&step.task_run_request_ref) {
            Some(r) => r.clone(),
            None => {
                eprintln!("[executor] {run_name}: missing task_run_request for step '{}'", step.name);
                append_run_log(registry, run_name, "error", format!("Missing task_run_request for step '{}'", step.name));
                all_ok = false;
                break;
            }
        };

        // Resolve any `work://steps/<step>/outputs?<name>` references at runtime.
        let req = resolve_step_inputs(req, &step_outputs);

        eprintln!(
            "[executor] run='{}' step='{}' runner='{}'",
            run_name, step.name, req.spec.execution.kind
        );
        append_run_log(registry, run_name, "info", format!("Step '{}' starting (runner: {})", step.name, req.spec.execution.kind));
        set_step_state(registry, run_name, &step.name, WorkRunState::Running);

        let task_doc = run_step(req);
        let ok = task_doc.as_ref()
            .and_then(|d| d.status.as_ref())
            .map(|s| s.state == TaskRunState::Succeeded)
            .unwrap_or(false);
        eprintln!("[executor] run='{}' step='{}' succeeded={ok}", run_name, step.name);

        if ok {
            append_run_log(registry, run_name, "info", format!("Step '{}' succeeded", step.name));
        } else {
            let stderr = task_doc.as_ref()
                .and_then(|d| d.status.as_ref())
                .and_then(|s| s.logs_ref.as_ref())
                .map(|l| l.stderr.trim())
                .filter(|s| !s.is_empty())
                .unwrap_or("no details captured");
            append_run_log(registry, run_name, "error",
                format!("Step '{}' failed — {}", step.name, stderr));
        }

        // Collect the step's outputs before saving so later steps can use them.
        if let Some(doc) = task_doc {
            step_outputs.insert(step.name.clone(), collect_step_outputs(&doc));
            let task_run_name = doc.name.clone();
            registry.insert_task_run(doc);
            set_step_task_run_ref(registry, run_name, &step.name, &task_run_name);
        }

        set_step_state(
            registry, run_name, &step.name,
            if ok { WorkRunState::Succeeded } else { WorkRunState::Failed },
        );

        if ok {
            completed.insert(step.name.clone());
        } else {
            all_ok = false;
            break;
        }
    }

    let final_state = if all_ok { WorkRunState::Succeeded } else { WorkRunState::Failed };
    eprintln!("[executor] run='{run_name}' final={final_state:?}");
    append_run_log(registry, run_name, "info", format!("Run finished: {:?}", final_state));
    set_run_state(registry, run_name, final_state);
}

// ─── Step dispatch ────────────────────────────────────────────────────────────

fn run_step(req: TaskRunRequestDoc) -> Option<TaskRunDoc> {
    let kind = req.spec.execution.kind.clone();
    let spec = TaskRunnerSpec { kind: kind.clone(), config: serde_json::Value::Null };
    if kind.contains("ShellTaskRunner") {
        run_with(ShellTaskRunner::new("exec", "exec", spec), req)
    } else if kind.contains("ContainerTaskRunner") {
        run_with(ContainerTaskRunner::new("exec", "exec", spec), req)
    } else if kind.contains("HttpTaskRunner") {
        run_with(HttpTaskRunner::new("exec", "exec", spec), req)
    } else if kind.contains("KubernetesTaskRunner") {
        run_with(KubernetesTaskRunner::new("exec", "exec", spec), req)
    } else {
        eprintln!("[executor] unknown runner kind: {kind}");
        None
    }
}

/// Spawn and start a task, then poll until it reaches a terminal state.
fn run_with(runner: impl TaskRunner, req: TaskRunRequestDoc) -> Option<TaskRunDoc> {
    let task = runner.spawn(req).ok()?;
    task.start().ok()?;

    loop {
        let doc = task.as_doc();
        if let Some(s) = &doc.status {
            if matches!(
                s.state,
                TaskRunState::Succeeded | TaskRunState::Failed | TaskRunState::Cancelled
            ) {
                return Some(doc);
            }
        }
        std::thread::sleep(Duration::from_millis(200));
    }
}

// ─── Registry helpers ─────────────────────────────────────────────────────────

fn set_step_state(registry: &WorkflowRegistry, run: &str, step: &str, state: WorkRunState) {
    if let Some(mut doc) = registry.get_work_run(run) {
        if let Some(status) = doc.status.as_mut() {
            if let Some(s) = status.steps.iter_mut().find(|s| s.name == step) {
                s.state = state;
            }
        }
        registry.update_work_run(doc);
    }
}

fn set_run_state(registry: &WorkflowRegistry, run: &str, state: WorkRunState) {
    use workaholic::WorkRunSummary;
    if let Some(mut doc) = registry.get_work_run(run) {
        if let Some(status) = doc.status.as_mut() {
            status.state       = state;
            status.finished_at = Some(chrono::Utc::now().to_rfc3339());
            // Recalculate summary from actual step states.
            let steps = &status.steps;
            status.summary = WorkRunSummary {
                total_steps:     steps.len(),
                pending_steps:   steps.iter().filter(|s| s.state == WorkRunState::Pending).count(),
                running_steps:   steps.iter().filter(|s| s.state == WorkRunState::Running).count(),
                succeeded_steps: steps.iter().filter(|s| s.state == WorkRunState::Succeeded).count(),
                failed_steps:    steps.iter().filter(|s| s.state == WorkRunState::Failed).count(),
                cancelled_steps: steps.iter().filter(|s| s.state == WorkRunState::Cancelled).count(),
            };
        }
        registry.update_work_run(doc);
    }
}

fn set_step_task_run_ref(registry: &WorkflowRegistry, run: &str, step: &str, task_run: &str) {
    if let Some(mut doc) = registry.get_work_run(run) {
        if let Some(status) = doc.status.as_mut() {
            if let Some(s) = status.steps.iter_mut().find(|s| s.name == step) {
                s.active_task_run_ref = Some(task_run.to_string());
            }
        }
        registry.update_work_run(doc);
    }
}
