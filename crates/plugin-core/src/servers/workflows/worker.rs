//! Worker — drives a Workflow through its full lifecycle, step by step.
//!
//! # Execution model
//!
//! 1. The Work definition is fetched from the Workspace server.
//! 2. All steps are initialised as `Pending`.
//! 3. The DAG is executed with Kahn's topological-sort algorithm:
//!    - All steps whose dependencies are satisfied form a "wave".
//!    - Every wave is executed in parallel via `tokio::task::JoinSet`.
//!    - When a step finishes its dependents are unlocked.
//! 4. The [`FailurePolicy`] controls how a failed step affects the rest:
//!    - `FailFast` — cancel everything and mark the Workflow `Failed`.
//!    - `ContinueOnFailure` — skip step's downstream; run independent branches.
//!    - `IgnoreFailures` — treat failure as success for dependency purposes.
//! 5. `task.spec.retries` controls how many times a step is re-attempted.
//! 6. `task.spec.timeout_seconds` and `workflow.execution.timeout_seconds`
//!    are honoured via `tokio::time::timeout`.
//!
//! # Task dispatch
//!
//! [`dispatch_task`] looks up the named [`TaskExecutor`] in the
//! [`ExecutorRegistry`] and delegates to it, converting results back to the
//! worker's `HashMap<String, Value>` output format.
//!
//! [`TaskExecutor`]: orkester_common::plugin::providers::executor::TaskExecutor
//! [`ExecutorRegistry`]: crate::executor::ExecutorRegistry

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use chrono::Utc;
use orkester_common::domain::{Task, Work, WorkStep};
use orkester_common::plugin::providers::executor::{ExecutionRequest, ExecutionStatus, ExecutorRegistry};
use serde_json::Value;
use uuid::Uuid;

use super::model::{
    FailurePolicy, StepState, StepStatus, Workflow, WorkflowMetrics, WorkflowStatus,
};
use super::store::WorkflowsStore;
use super::workspace_client::WorkspaceClient;

// ── Worker trait ──────────────────────────────────────────────────────────────

#[async_trait]
pub trait Worker: Send + Sync {
    /// Drive `workflow` to completion (or failure/cancellation).
    async fn run(&self, workflow: Workflow, store: WorkflowsStore, workspace: WorkspaceClient);

    /// Gracefully cancel the given workflow.
    async fn cancel(&self, workflow_id: &str, namespace: &str, store: WorkflowsStore);
}

// ── LocalWorker ───────────────────────────────────────────────────────────────

pub struct LocalWorker {
    pub executor_registry: Arc<ExecutorRegistry>,
}

#[async_trait]
impl Worker for LocalWorker {
    async fn run(&self, mut workflow: Workflow, store: WorkflowsStore, workspace: WorkspaceClient) {
        orkester_common::log_info!(
            "Worker: starting workflow '{}' ({}/{}@{})",
            workflow.id,
            workflow.namespace,
            workflow.work_name,
            workflow.work_version
        );

        // ── 1. Transition to Running ──────────────────────────────────────
        workflow.status = WorkflowStatus::Running;
        workflow.started_at = Some(Utc::now());
        workflow.updated_at = Utc::now();
        if let Err(e) = store.put_workflow(&workflow).await {
            orkester_common::log_error!(
                "Worker: failed to persist Running state for '{}': {}",
                workflow.id,
                e
            );
            return;
        }

        // ── 2. Resolve Work definition ────────────────────────────────────
        let work = match workspace
            .get_work(
                &workflow.namespace,
                &workflow.work_name,
                &workflow.work_version,
            )
            .await
        {
            Ok(w) => w,
            Err(e) => {
                fail_workflow(
                    &mut workflow,
                    format!("could not fetch Work definition: {e}"),
                    &store,
                )
                .await;
                return;
            }
        };

        // ── 3. Initialise step states ─────────────────────────────────────
        workflow.metrics.steps_total = work.spec.steps.len() as u32;
        for step in &work.spec.steps {
            workflow.steps.insert(
                step.id.clone(),
                StepState {
                    status: StepStatus::Pending,
                    started_at: None,
                    finished_at: None,
                    outputs: HashMap::new(),
                    error: None,
                    attempt: 1,
                },
            );
        }
        if let Err(e) = store.put_workflow(&workflow).await {
            orkester_common::log_error!(
                "Worker: failed to persist initial step states for '{}': {}",
                workflow.id,
                e
            );
            return;
        }

        // ── 4. Execute the DAG (with optional whole-workflow timeout) ─────
        let timeout = workflow.execution.timeout_seconds.map(Duration::from_secs);
        let executors = Arc::clone(&self.executor_registry);
        let result = if let Some(t) = timeout {
            tokio::time::timeout(
                t,
                execute_dag(&mut workflow, &work, &store, &workspace, &executors),
            )
            .await
            .unwrap_or_else(|_| Err("workflow timed out".to_string()))
        } else {
            execute_dag(&mut workflow, &work, &store, &workspace, &executors).await
        };

        // ── 5. Final state ────────────────────────────────────────────────
        let now = Utc::now();
        workflow.status = if result.is_ok() {
            WorkflowStatus::Succeeded
        } else {
            WorkflowStatus::Failed
        };
        workflow.finished_at = Some(now);
        workflow.updated_at = now;
        if let Some(start) = workflow.started_at {
            workflow.metrics.duration_seconds =
                Some((now - start).num_milliseconds() as f64 / 1000.0);
        }

        orkester_common::log_info!(
            "Worker: workflow '{}' finished as {:?}",
            workflow.id,
            workflow.status
        );

        if let Err(e) = store.put_workflow(&workflow).await {
            orkester_common::log_error!(
                "Worker: failed to persist final state for '{}': {}",
                workflow.id,
                e
            );
        }
    }

    async fn cancel(&self, workflow_id: &str, namespace: &str, store: WorkflowsStore) {
        match store.get_workflow(namespace, workflow_id).await {
            Ok(mut wf) => {
                if wf.status.is_terminal() {
                    orkester_common::log_warn!(
                        "Worker: workflow '{}' is already terminal ({}), ignoring cancel.",
                        workflow_id,
                        wf.status
                    );
                    return;
                }
                // The DAG loop polls for Cancelled status between waves, so setting it
                // here is sufficient for a graceful stop.
                wf.status = WorkflowStatus::Cancelled;
                wf.finished_at = Some(Utc::now());
                wf.updated_at = Utc::now();
                if let Err(e) = store.put_workflow(&wf).await {
                    orkester_common::log_error!(
                        "Worker: failed to persist Cancelled state for '{}': {}",
                        workflow_id,
                        e
                    );
                }
            }
            Err(e) => orkester_common::log_error!(
                "Worker: workflow '{}' not found during cancel: {}",
                workflow_id,
                e
            ),
        }
    }
}

// ── DAG execution ─────────────────────────────────────────────────────────────

async fn execute_dag(
    workflow: &mut Workflow,
    work: &Work,
    store: &WorkflowsStore,
    workspace: &WorkspaceClient,
    executors: &Arc<ExecutorRegistry>,
) -> Result<(), String> {
    // Build lookup and adjacency structures once.
    let step_map: HashMap<String, WorkStep> = work
        .spec
        .steps
        .iter()
        .map(|s| (s.id.clone(), s.clone()))
        .collect();

    let mut in_degree: HashMap<String, usize> = step_map
        .iter()
        .map(|(id, s)| (id.clone(), s.depends_on.len()))
        .collect();

    // dependents[x] = list of steps that list x in their depends_on
    let mut dependents: HashMap<String, Vec<String>> = HashMap::new();
    for step in &work.spec.steps {
        for dep in &step.depends_on {
            dependents
                .entry(dep.clone())
                .or_default()
                .push(step.id.clone());
        }
    }

    // Seed the queue with all root steps (no dependencies).
    let mut ready: VecDeque<String> = in_degree
        .iter()
        .filter(|(_, &deg)| deg == 0)
        .map(|(id, _)| id.clone())
        .collect();

    // Outputs collected from completed steps, forwarded to dependents as inputs.
    let mut step_outputs: HashMap<String, HashMap<String, Value>> = HashMap::new();
    let mut failed_any = false;

    while !ready.is_empty() {
        // Poll for external cancellation before every wave.
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

        // Mark every step in the wave as Running and persist before spawning.
        for step_id in &wave {
            if let Some(state) = workflow.steps.get_mut(step_id) {
                state.status = StepStatus::Running;
                state.started_at = Some(Utc::now());
            }
        }
        let _ = store.put_workflow(workflow).await;

        // Execute all steps in the wave in parallel.
        let mut join_set: tokio::task::JoinSet<(String, Result<HashMap<String, Value>, String>)> =
            tokio::task::JoinSet::new();

        for step_id in &wave {
            let step = step_map[step_id].clone();
            let inputs = build_step_inputs(&step, &workflow.work_context, &step_outputs);
            let workspace_c = workspace.clone();
            let executors_c = Arc::clone(executors);
            let ns = workflow.namespace.clone();

            join_set.spawn(async move {
                let result = execute_step(&step, &ns, inputs, &workspace_c, &executors_c).await;
                (step.id.clone(), result)
            });
        }

        // Collect wave results.
        let mut wave_results: Vec<(String, Result<HashMap<String, Value>, String>)> = Vec::new();
        while let Some(join_res) = join_set.join_next().await {
            match join_res {
                Ok(r) => wave_results.push(r),
                Err(e) => wave_results.push(("unknown".to_string(), Err(e.to_string()))),
            }
        }

        // Process each result, update step states, apply failure policy.
        for (step_id, result) in wave_results {
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
                    unlock_dependents(
                        &step_id,
                        &dependents,
                        &mut in_degree,
                        &workflow.steps,
                        &mut ready,
                    );
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
                            // Treat as success for dependency purposes.
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

/// Decrement in-degree for each dependent and add to `ready` if it reaches 0
/// and the step is still `Pending`.
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
                let still_pending = steps
                    .get(dep_id)
                    .map(|s| s.status == StepStatus::Pending)
                    .unwrap_or(false);
                if still_pending {
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

// ── Step execution ────────────────────────────────────────────────────────────

/// Execute a single step: resolve its Task, apply retries and per-task timeout.
async fn execute_step(
    step: &WorkStep,
    namespace: &str,
    inputs: HashMap<String, Value>,
    workspace: &WorkspaceClient,
    executors: &Arc<ExecutorRegistry>,
) -> Result<HashMap<String, Value>, String> {
    // Resolve the Task definition by name within the namespace.
    let tasks = workspace
        .list_tasks(namespace)
        .await
        .map_err(|e| format!("could not list tasks in '{namespace}': {e}"))?;
    let task = tasks
        .into_iter()
        .find(|t| t.meta.name == step.task)
        .ok_or_else(|| format!("task '{}' not found in namespace '{namespace}'", step.task))?;

    let max_attempts = task.spec.retries + 1;
    let timeout = task.spec.timeout_seconds.map(Duration::from_secs);

    for attempt in 1..=max_attempts {
        orkester_common::log_info!(
            "Worker: step '{}' → task '{}' (executor: '{}', attempt {}/{})",
            step.id,
            task.meta.name,
            task.spec.executor,
            attempt,
            max_attempts
        );

        let result = match timeout {
            Some(t) => tokio::time::timeout(t, dispatch_task(&task, inputs.clone(), executors))
                .await
                .unwrap_or_else(|_| {
                    Err(format!(
                        "task '{}' timed out after {}s",
                        task.meta.name,
                        task.spec.timeout_seconds.unwrap_or(0)
                    ))
                }),
            None => dispatch_task(&task, inputs.clone(), executors).await,
        };

        match result {
            Ok(outputs) => return Ok(outputs),
            Err(msg) if attempt < max_attempts => {
                orkester_common::log_warn!(
                    "Worker: step '{}' attempt {}/{} failed: {} — retrying",
                    step.id,
                    attempt,
                    max_attempts,
                    msg
                );
            }
            Err(msg) => return Err(msg),
        }
    }

    unreachable!()
}

/// Merge inputs for a step: workflow context < dependency outputs < static overrides.
fn build_step_inputs(
    step: &WorkStep,
    work_context: &HashMap<String, Value>,
    step_outputs: &HashMap<String, HashMap<String, Value>>,
) -> HashMap<String, Value> {
    let mut inputs: HashMap<String, Value> = work_context.clone();

    // Prefix dependency outputs with "<dep_id>." to avoid collisions.
    for dep_id in &step.depends_on {
        if let Some(outputs) = step_outputs.get(dep_id) {
            for (k, v) in outputs {
                inputs.insert(format!("{dep_id}.{k}"), v.clone());
            }
        }
    }

    // Static overrides from the WorkStep definition have the highest priority.
    for (k, v) in &step.inputs {
        inputs.insert(k.clone(), Value::String(v.clone()));
    }

    inputs
}

/// Look up the named executor in the registry and run the task.
async fn dispatch_task(
    task: &Task,
    inputs: HashMap<String, Value>,
    executors: &ExecutorRegistry,
) -> Result<HashMap<String, Value>, String> {
    let executor = executors.get(&task.spec.executor).ok_or_else(|| {
        format!(
            "no executor registered for '{}' (task '{}')",
            task.spec.executor, task.meta.name
        )
    })?;

    let request = ExecutionRequest {
        id: Uuid::new_v4().to_string(),
        task_definition: task.spec.config.clone(),
        inputs,
    };

    let result = executor
        .execute(request)
        .await
        .map_err(|e| e.to_string())?;

    match result.status {
        ExecutionStatus::Succeeded => Ok(result.outputs),
        ExecutionStatus::Failed(msg) => Err(msg),
        other => Err(format!("unexpected execution status: {other:?}")),
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

async fn fail_workflow(workflow: &mut Workflow, reason: String, store: &WorkflowsStore) {
    let now = Utc::now();
    orkester_common::log_error!("Worker: workflow '{}' failed — {}", workflow.id, reason);
    workflow.status = WorkflowStatus::Failed;
    workflow.finished_at = Some(now);
    workflow.updated_at = now;
    if let Some(start) = workflow.started_at {
        workflow.metrics.duration_seconds = Some((now - start).num_milliseconds() as f64 / 1000.0);
    }
    let _ = store.put_workflow(workflow).await;
}
