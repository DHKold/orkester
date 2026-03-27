//! RunnerServer — a component that bridges the host dispatch mechanism to the
//! internal [`TaskRunner`] trait implementations.
//!
//! ## Design
//!
//! WorkRunners call the host with action `runner/Execute`.  Because the component
//! kind `workaholic/RunnerServer:1.0` contains the word "runner", the host's
//! namespace-based action router forwards the request here.  The component is
//! stateless: each call is fully independent, making concurrent invocations from
//! multiple workRunner threads safe in practice even though the SDK dispatch takes
//! `&mut self` (there is no mutable shared state to race on).
//!
//! ## Future
//!
//! Once the host supports runtime `orkester/CreateComponent` callbacks from
//! plugins, individual runner kinds (`workaholic/ShellRunner:1.0`, etc.) can be
//! created on demand and freed after use, eliminating the single shared instance.

use orkester_plugin::prelude::*;
use serde::{Deserialize, Serialize};
use workaholic::{domain::task::ExecutionKind, execution::task_run::{TaskRunError, TaskRunPhase}};

use super::build_runner;

// ── Wire types (owned by RunnerServer for backward compat) ────────────────────

/// Sent from a workRunner to the `RunnerServer` component via `runner/Execute`.
#[derive(Debug, Deserialize)]
pub struct RunnerExecuteRequest {
    /// Execution backend kind (shell, container, kubernetes, …).
    pub kind: ExecutionKind,
    /// Human-readable name for the task being executed (used in logs).
    pub task_name: String,
    /// Resolved execution config: merged task spec + work-task inputs.
    pub inputs: serde_json::Value,
}

/// Response returned by the `RunnerServer` once execution is complete.
#[derive(Debug, Serialize)]
pub struct RunnerExecuteResponse {
    pub phase: TaskRunPhase,
    pub outputs: serde_json::Value,
    pub external_id: Option<String>,
    pub error: Option<TaskRunError>,
}

// ── RunnerServer ──────────────────────────────────────────────────────────────

/// Host-registered component that executes tasks by delegating to the
/// appropriate [`TaskRunner`] backend (shell, container, kubernetes, …).
///
/// **Registration** — add this to the orkester config `servers` list:
/// ```yaml
/// - name: runner-server
///   kind: workaholic/RunnerServer:1.0
/// ```
pub struct RunnerServer;

#[component(
    kind        = "workaholic/RunnerServer:1.0",
    name        = "RunnerServer",
    description = "Executes workflow tasks via the appropriate runner backend."
)]
impl RunnerServer {
    /// Synchronously execute a task and return the result.
    ///
    /// The call blocks the caller's thread until the task process finishes,
    /// times out, or is cancelled.  WorkRunners call this on their dedicated
    /// background thread so blocking is expected and desirable.
    #[handle("runner/Execute")]
    fn execute(&mut self, req: RunnerExecuteRequest) -> Result<RunnerExecuteResponse> {
        let mut runner = build_runner(&req.kind);
        let handle = runner.spawn(&req.task_name, &req.inputs);
        let result = handle.wait();
        Ok(RunnerExecuteResponse {
            phase:       result.phase,
            outputs:     result.outputs,
            external_id: result.external_id,
            error:       result.error,
        })
    }
}
