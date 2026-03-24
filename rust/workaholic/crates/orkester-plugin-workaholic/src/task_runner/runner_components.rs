//! On-demand task runner components.
//!
//! Each runner kind is a separate component type that is created fresh for
//! **each task execution** via `host.create_component(kind, config)`:
//!
//! | Component kind                    | Creates         |
//! |-----------------------------------|-----------------|
//! | `workaholic/ShellRunner:1.0`      | `ShellRunnerServer`      |
//! | `workaholic/ContainerRunner:1.0`  | `ContainerRunnerServer`  |
//! | `workaholic/KubernetesRunner:1.0` | `KubernetesRunnerServer` |
//!
//! Because these components are **transient** (not registered in the host
//! routing table), their action does not need a namespace prefix.  The worker
//! calls `owned_component.call("Execute", req)` directly on the pointer.

use orkester_plugin::prelude::*;
use serde::{Deserialize, Serialize};
use workaholic::execution::task_run::{TaskRunError, TaskRunPhase};

// ── Shared request / response ─────────────────────────────────────────────────

/// Request payload for the `Execute` action on any runner component.
#[derive(Debug, Serialize, Deserialize)]
pub struct RunnerExecuteRequest {
    /// Human-readable task name (for log context).
    pub task_name: String,
    /// Resolved execution config: merged task spec + work-task inputs.
    pub inputs: serde_json::Value,
}

/// Result returned by a runner once execution completes.
#[derive(Debug, Serialize, Deserialize)]
pub struct RunnerExecuteResponse {
    pub phase: TaskRunPhase,
    pub outputs: serde_json::Value,
    pub external_id: Option<String>,
    pub error: Option<TaskRunError>,
}

// ── Helper ────────────────────────────────────────────────────────────────────

fn run_to_completion(
    mut runner: Box<dyn super::TaskRunner>,
    req: RunnerExecuteRequest,
) -> std::result::Result<RunnerExecuteResponse, Error> {
    let handle = runner.spawn(&req.task_name, &req.inputs);
    let result = handle.wait();
    Ok(RunnerExecuteResponse {
        phase:       result.phase,
        outputs:     result.outputs,
        external_id: result.external_id,
        error:       result.error,
    })
}

// ── ShellRunnerServer ─────────────────────────────────────────────────────────

/// Config for `workaholic/ShellRunner:1.0` — none required.
#[derive(Debug, Default, Deserialize)]
pub struct ShellRunnerConfig {}

/// Executes a task via a local `sh` process.
pub struct ShellRunnerServer;

#[component(
    kind        = "workaholic/ShellRunner:1.0",
    name        = "ShellRunner",
    description = "Executes workflow tasks via a local shell (sh -c)."
)]
impl ShellRunnerServer {
    #[handle("Execute")]
    fn execute(&mut self, req: RunnerExecuteRequest) -> Result<RunnerExecuteResponse> {
        run_to_completion(Box::new(super::shell::ShellTaskRunner::new()), req)
    }
}

// ── ContainerRunnerServer ─────────────────────────────────────────────────────

/// Config for `workaholic/ContainerRunner:1.0` — none required.
#[derive(Debug, Default, Deserialize)]
pub struct ContainerRunnerConfig {}

/// Executes a task inside a container (Docker/Podman).
pub struct ContainerRunnerServer;

#[component(
    kind        = "workaholic/ContainerRunner:1.0",
    name        = "ContainerRunner",
    description = "Executes workflow tasks inside a container (Docker/Podman)."
)]
impl ContainerRunnerServer {
    #[handle("Execute")]
    fn execute(&mut self, req: RunnerExecuteRequest) -> Result<RunnerExecuteResponse> {
        run_to_completion(Box::new(super::container::ContainerTaskRunner::new()), req)
    }
}

// ── KubernetesRunnerServer ────────────────────────────────────────────────────

/// Config for `workaholic/KubernetesRunner:1.0` — none required.
#[derive(Debug, Default, Deserialize)]
pub struct KubernetesRunnerConfig {}

/// Executes a task as a Kubernetes Job.
pub struct KubernetesRunnerServer;

#[component(
    kind        = "workaholic/KubernetesRunner:1.0",
    name        = "KubernetesRunner",
    description = "Executes workflow tasks as Kubernetes Jobs."
)]
impl KubernetesRunnerServer {
    #[handle("Execute")]
    fn execute(&mut self, req: RunnerExecuteRequest) -> Result<RunnerExecuteResponse> {
        run_to_completion(Box::new(super::kubernetes::KubernetesTaskRunner::new()), req)
    }
}
