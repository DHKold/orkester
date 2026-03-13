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
/// `task` is already resolved — no workspace lookup is performed here.
pub(super) async fn execute(
    step: &WorkStep,
    task: Task,
    inputs: HashMap<String, Value>,
    executors: &Arc<ExecutorRegistry>,
) -> Result<HashMap<String, Value>, String> {
    let max_attempts = task.spec.retries + 1;
    let timeout = task.spec.timeout_seconds.map(Duration::from_secs);

    for attempt in 1..=max_attempts {
        orkester_common::log_info!(
            "Worker: step '{}' → task '{}' (executor: '{}', attempt {}/{})",
            step.id,
            task.meta.name,
            task.spec.executor,
            attempt,
            max_attempts,
        );

        let outcome = match timeout {
            Some(t) => {
                tokio::time::timeout(t, dispatch(&task, inputs.clone(), executors))
                    .await
                    .unwrap_or_else(|_| {
                        Err(format!(
                            "task '{}' timed out after {}s",
                            task.meta.name,
                            task.spec.timeout_seconds.unwrap_or(0),
                        ))
                    })
            }
            None => dispatch(&task, inputs.clone(), executors).await,
        };

        match outcome {
            Ok(outputs) => return Ok(outputs),
            Err(msg) if attempt < max_attempts => {
                orkester_common::log_warn!(
                    "Worker: step '{}' attempt {}/{} failed: {} — retrying",
                    step.id,
                    attempt,
                    max_attempts,
                    msg,
                );
            }
            Err(msg) => return Err(msg),
        }
    }

    unreachable!("loop exhausted without returning")
}

/// Build the merged input map for a step.
///
/// Priority (lowest → highest):
/// 1. Workflow-level `work_context`
/// 2. Outputs from upstream dependency steps (keyed as `"<dep_id>.<output_key>"`)
/// 3. Step-level static `inputs` overrides
pub(super) fn build_inputs(
    step: &WorkStep,
    work_context: &HashMap<String, Value>,
    step_outputs: &HashMap<String, HashMap<String, Value>>,
) -> HashMap<String, Value> {
    let mut inputs = work_context.clone();

    for dep_id in &step.depends_on {
        if let Some(outputs) = step_outputs.get(dep_id) {
            for (k, v) in outputs {
                inputs.insert(format!("{dep_id}.{k}"), v.clone());
            }
        }
    }

    for (k, v) in &step.inputs {
        inputs.insert(k.clone(), Value::String(v.clone()));
    }

    inputs
}

// ── Internal dispatch ─────────────────────────────────────────────────────────

async fn dispatch(
    task: &Task,
    inputs: HashMap<String, Value>,
    executors: &ExecutorRegistry,
) -> Result<HashMap<String, Value>, String> {
    let executor = executors.get(&task.spec.executor).ok_or_else(|| {
        format!(
            "no executor registered for '{}' (task '{}')",
            task.spec.executor, task.meta.name,
        )
    })?;

    let result = executor
        .execute(ExecutionRequest {
            id: Uuid::new_v4().to_string(),
            task_definition: task.spec.config.clone(),
            inputs,
        })
        .await
        .map_err(|e| e.to_string())?;

    for line in &result.logs {
        orkester_common::log_info!("[{}] {}", task.meta.name, line);
    }

    match result.status {
        ExecutionStatus::Succeeded => Ok(result.outputs),
        ExecutionStatus::Failed(msg) => Err(msg),
        other => Err(format!("unexpected execution status: {other:?}")),
    }
}
