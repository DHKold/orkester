//! Per-step execution: retry loop, timeout, input merging, and executor dispatch.
//!
//! The [`Task`] definition is provided by the caller (pre-resolved by the DAG
//! engine) so this module has no dependency on the workspace or store.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use orkester_common::domain::{Task, WorkStep};
use orkester_common::plugin::providers::executor::{
    ExecutionRequest, ExecutionStatus, ExecutorRegistry,
};
use serde_json::Value;
use uuid::Uuid;

// ── Public API ────────────────────────────────────────────────────────────────

/// Execute a single workflow step, applying the task's retry and timeout policy.
///
/// Returns `(result, logs)` — logs are accumulated across all retry attempts
/// and are always present regardless of success or failure.
///
/// `task` is already resolved — no workspace lookup is performed here.
pub(super) async fn execute(
    step: &WorkStep,
    task: Task,
    inputs: HashMap<String, Value>,
    executors: &Arc<ExecutorRegistry>,
) -> (Result<HashMap<String, Value>, String>, Vec<String>) {
    let max_attempts = task.spec.retries + 1;
    let timeout = task.spec.timeout_seconds.map(Duration::from_secs);

    let mut all_logs: Vec<String> = Vec::new();

    for attempt in 1..=max_attempts {
        orkester_common::log_info!(
            "Worker: step '{}' → task '{}' (executor: '{}', attempt {}/{})",
            step.id,
            task.meta.name,
            task.spec.executor,
            attempt,
            max_attempts,
        );

        let (outcome, logs) = match timeout {
            Some(t) => {
                tokio::time::timeout(t, dispatch(&task, inputs.clone(), executors))
                    .await
                    .unwrap_or_else(|_| (
                        Err(format!(
                            "task '{}' timed out after {}s",
                            task.meta.name,
                            task.spec.timeout_seconds.unwrap_or(0),
                        )),
                        vec![],
                    ))
            }
            None => dispatch(&task, inputs.clone(), executors).await,
        };
        all_logs.extend(logs);

        match outcome {
            Ok(outputs) => return (Ok(outputs), all_logs),
            Err(msg) if attempt < max_attempts => {
                orkester_common::log_warn!(
                    "Worker: step '{}' attempt {}/{} failed: {} — retrying",
                    step.id,
                    attempt,
                    max_attempts,
                    msg,
                );
            }
            Err(msg) => return (Err(msg), all_logs),
        }
    }

    unreachable!("loop exhausted without returning")
}

/// Build the merged input map for a step.
///
/// Priority (highest to lowest):
/// 1. Workflow-level `work_context`
/// 2. Outputs from upstream dependency steps (keyed as `"<dep_id>.<output_key>"`)
/// 3. Step-level static `inputs` overrides
pub(super) fn build_inputs(
    step: &WorkStep,
    work_context: &HashMap<String, Value>,
    step_outputs: &HashMap<String, HashMap<String, Value>>,
) -> HashMap<String, Value> {
    // Start with step inputs
    let mut inputs: HashMap<String, Value> = HashMap::new();
    for (k, v) in &step.inputs {
        inputs.insert(k.clone(), Value::String(v.clone()));
    }

    // Overlay with outputs from dependencies, keyed as "<dep_id>.<output_key>"
    for dep_id in &step.depends_on {
        if let Some(outputs) = step_outputs.get(dep_id) {
            for (k, v) in outputs {
                inputs.insert(k.clone(), v.clone());
            }
        }
    }

    // Overlay with workflow-level context
    for (k, v) in work_context {
        inputs.insert(k.clone(), v.clone());
    }

    inputs
}

// ── Internal dispatch ─────────────────────────────────────────────────────────

async fn dispatch(
    task: &Task,
    inputs: HashMap<String, Value>,
    executors: &ExecutorRegistry,
) -> (Result<HashMap<String, Value>, String>, Vec<String>) {
    let executor = match executors.get(&task.spec.executor) {
        Some(e) => e,
        None => return (
            Err(format!(
                "no executor registered for '{}' (task '{}')",
                task.spec.executor, task.meta.name,
            )),
            vec![],
        ),
    };

    let result = match executor
        .execute(ExecutionRequest {
            id: Uuid::new_v4().to_string(),
            task_definition: task.spec.config.clone(),
            inputs,
            outputs: task.spec.outputs.keys().cloned().collect(),
        })
        .await
    {
        Err(e) => return (Err(e.to_string()), vec![]),
        Ok(r) => r,
    };

    for line in &result.logs {
        orkester_common::log_info!("[{}] {}", task.meta.name, line);
    }
    let logs = result.logs;

    let outcome = match result.status {
        ExecutionStatus::Succeeded => Ok(result.outputs),
        ExecutionStatus::Failed(msg) => Err(msg),
        other => Err(format!("unexpected execution status: {other:?}")),
    };
    (outcome, logs)
}
