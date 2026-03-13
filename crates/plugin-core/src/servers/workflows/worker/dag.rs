//! DAG execution engine — Kahn's topological-sort algorithm.
//!
//! Steps are executed in *waves*: all steps whose dependencies have been
//! satisfied form a wave and are launched in parallel via
//! [`tokio::task::JoinSet`].  When every step in a wave finishes the next
//! wave is seeded from the newly unlocked dependents.
//!
//! # Task pre-fetching
//!
//! Before the first wave this module fetches all [`Task`] definitions for the
//! workflow's namespace in a **single** workspace round-trip and builds a local
//! map.  Steps therefore never block on workspace I/O during execution.
//!
//! [`Task`]: orkester_common::domain::Task

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

use chrono::Utc;
use orkester_common::domain::{Task, Work};
use orkester_common::plugin::providers::executor::ExecutorRegistry;
use serde_json::Value;

use super::super::model::{
    FailurePolicy, StepState, StepStatus, Workflow, WorkflowMetrics, WorkflowStatus,
};
use super::super::store::WorkflowsStore;
use super::super::workspace_client::WorkspaceClient;
use super::step;

// ── Public entry-point ────────────────────────────────────────────────────────

/// Execute the full DAG for `workflow` given its `work` definition.
///
/// Returns `Ok(())` when all steps succeeded (or failures were ignored), or an
/// `Err(reason)` string when execution should be marked as `Failed`.
pub(super) async fn execute(
    workflow: &mut Workflow,
    work: &Work,
    store: &WorkflowsStore,
    workspace: &WorkspaceClient,
    executors: &Arc<ExecutorRegistry>,
) -> Result<(), String> {
    // ── Pre-fetch all tasks (single round-trip) ───────────────────────────
    let task_map: HashMap<String, Task> = workspace
        .list_tasks(&workflow.namespace)
        .await
        .map_err(|e| format!("could not list tasks in '{}': {e}", workflow.namespace))?
        .into_iter()
        .map(|t| (t.meta.name.clone(), t))
        .collect();

    // ── Build adjacency structures ────────────────────────────────────────
    let step_map: HashMap<String, _> = work
        .spec
        .steps
        .iter()
        .map(|s| (s.id.clone(), s.clone()))
        .collect();

    let mut in_degree: HashMap<String, usize> = step_map
        .iter()
        .map(|(id, s)| (id.clone(), s.depends_on.len()))
        .collect();

    // dependents[x] = all steps that depend on x
    let mut dependents: HashMap<String, Vec<String>> = HashMap::new();
    for step in &work.spec.steps {
        for dep in &step.depends_on {
            dependents
                .entry(dep.clone())
                .or_default()
                .push(step.id.clone());
        }
    }

    // Seed with all root steps (no incoming edges).
    let mut ready: VecDeque<String> = in_degree
        .iter()
        .filter(|(_, &deg)| deg == 0)
        .map(|(id, _)| id.clone())
        .collect();

    // Accumulated outputs from completed steps, keyed by step id.
    let mut step_outputs: HashMap<String, HashMap<String, Value>> = HashMap::new();
    let mut failed_any = false;

    // ── Wave loop ─────────────────────────────────────────────────────────
    while !ready.is_empty() {
        // Check for external cancellation before each wave.
        if let Ok(fresh) = store.get_workflow(&workflow.namespace, &workflow.id).await {
            if fresh.status == WorkflowStatus::Cancelled {
                for state in workflow.steps.values_mut() {
                    if state.status == StepStatus::Pending {
                        state.status = StepStatus::Cancelled;
                    }
                }
                let _ = store.put_workflow(workflow).await;
                return Err("cancelled externally".to_string());
            }
        }

        let wave: Vec<String> = ready.drain(..).collect();

        // Transition every step in the wave to Running before spawning.
        let wave_start = Utc::now();
        for step_id in &wave {
            if let Some(state) = workflow.steps.get_mut(step_id) {
                state.status = StepStatus::Running;
                state.started_at = Some(wave_start);
            }
        }
        let _ = store.put_workflow(workflow).await;

        // Spawn all steps in the wave in parallel.
        let mut join_set: tokio::task::JoinSet<(String, Result<HashMap<String, Value>, String>)> =
            tokio::task::JoinSet::new();

        for step_id in &wave {
            let wstep = step_map[step_id].clone();
            let task = match task_map.get(&wstep.task).cloned() {
                Some(t) => t,
                None => {
                    let id = step_id.clone();
                    let name = wstep.task.clone();
                    join_set.spawn(async move {
                        (id, Err(format!("task '{name}' not found in workspace")))
                    });
                    continue;
                }
            };
            let inputs = step::build_inputs(&wstep, &workflow.work_context, &step_outputs);
            let executors_c = Arc::clone(executors);
            join_set.spawn(async move {
                let result = step::execute(&wstep, task, inputs, &executors_c).await;
                (wstep.id.clone(), result)
            });
        }

        // Collect results and update workflow state.
        while let Some(join_res) = join_set.join_next().await {
            let (step_id, result) = join_res.unwrap_or_else(|e| ("unknown".into(), Err(e.to_string())));
            let now = Utc::now();
            match result {
                Ok(outputs) => {
                    if let Some(state) = workflow.steps.get_mut(&step_id) {
                        state.status = StepStatus::Succeeded;
                        state.finished_at = Some(now);
                        state.outputs = outputs.clone();
                    }
                    step_outputs.insert(step_id.clone(), outputs);
                    workflow.metrics.steps_succeeded += 1;
                    unlock_dependents(&step_id, &dependents, &mut in_degree, &workflow.steps, &mut ready);
                }
                Err(msg) => {
                    if let Some(state) = workflow.steps.get_mut(&step_id) {
                        state.status = StepStatus::Failed;
                        state.finished_at = Some(now);
                        state.error = Some(msg.clone());
                    }
                    workflow.metrics.steps_failed += 1;

                    match workflow.execution.failure_policy {
                        FailurePolicy::FailFast => {
                            for state in workflow.steps.values_mut() {
                                if state.status == StepStatus::Pending {
                                    state.status = StepStatus::Cancelled;
                                    workflow.metrics.steps_skipped += 1;
                                }
                            }
                            let _ = store.put_workflow(workflow).await;
                            return Err(format!("step '{step_id}' failed: {msg}"));
                        }
                        FailurePolicy::ContinueOnFailure => {
                            failed_any = true;
                            mark_downstream_skipped(
                                &step_id,
                                &dependents,
                                &mut workflow.steps,
                                &mut workflow.metrics,
                            );
                        }
                        FailurePolicy::IgnoreFailures => {
                            // Treat the failure as success for dependency purposes.
                            step_outputs.insert(step_id.clone(), HashMap::new());
                            unlock_dependents(
                                &step_id,
                                &dependents,
                                &mut in_degree,
                                &workflow.steps,
                                &mut ready,
                            );
                        }
                    }
                }
            }
        }

        let _ = store.put_workflow(workflow).await;
    }

    if failed_any {
        Err("one or more steps failed".to_string())
    } else {
        Ok(())
    }
}

/// Transition a workflow directly to `Failed` and persist it.
/// Used for errors that occur outside the DAG loop (e.g. Work not found).
pub(super) async fn fail_workflow(workflow: &mut Workflow, reason: String, store: &WorkflowsStore) {
    let now = Utc::now();
    orkester_common::log_error!("Worker: workflow '{}' failed — {}", workflow.id, reason);
    workflow.status = WorkflowStatus::Failed;
    workflow.finished_at = Some(now);
    workflow.updated_at = now;
    if let Some(start) = workflow.started_at {
        workflow.metrics.duration_seconds =
            Some((now - start).num_milliseconds() as f64 / 1000.0);
    }
    let _ = store.put_workflow(workflow).await;
}

// ── DAG helpers ───────────────────────────────────────────────────────────────

/// Decrement the in-degree of each dependent of `step_id`.
/// Steps that reach in-degree 0 and are still `Pending` are pushed to `ready`.
fn unlock_dependents(
    step_id: &str,
    dependents: &HashMap<String, Vec<String>>,
    in_degree: &mut HashMap<String, usize>,
    steps: &HashMap<String, StepState>,
    ready: &mut VecDeque<String>,
) {
    for dep_id in dependents.get(step_id).into_iter().flatten() {
        if let Some(deg) = in_degree.get_mut(dep_id) {
            *deg -= 1;
            if *deg == 0 {
                if steps.get(dep_id).map(|s| s.status == StepStatus::Pending).unwrap_or(false) {
                    ready.push_back(dep_id.clone());
                }
            }
        }
    }
}

/// Recursively mark all transitive downstream steps as `Skipped`.
fn mark_downstream_skipped(
    step_id: &str,
    dependents: &HashMap<String, Vec<String>>,
    steps: &mut HashMap<String, StepState>,
    metrics: &mut WorkflowMetrics,
) {
    for dep_id in dependents
        .get(step_id)
        .into_iter()
        .flatten()
        .cloned()
        .collect::<Vec<_>>()
    {
        if let Some(state) = steps.get_mut(&dep_id) {
            if state.status == StepStatus::Pending {
                state.status = StepStatus::Skipped;
                metrics.steps_skipped += 1;
                mark_downstream_skipped(&dep_id, dependents, steps, metrics);
            }
        }
    }
}
