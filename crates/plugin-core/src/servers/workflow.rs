use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use async_trait::async_trait;
use serde_json::Value;
use tokio::sync::{mpsc, RwLock};
use tracing::{info, warn};
use orkester_common::domain::{
    ExecutionId, TaskExecution, TaskExecutionStatus, WorkExecution, WorkExecutionStatus,
};
use orkester_common::servers::workflow::{
    ExecutionRequest, WorkflowError, WorkflowHandle, WorkflowServer, WorkflowServerFactory,
};

// ── ID generation ─────────────────────────────────────────────────────────────

static EXEC_COUNTER: AtomicU64 = AtomicU64::new(1);

fn next_exec_id() -> ExecutionId {
    ExecutionId(format!("exec-{}", EXEC_COUNTER.fetch_add(1, Ordering::Relaxed)))
}

// ── Handle ────────────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct BasicWorkflowHandle {
    tx: mpsc::Sender<ExecutionRequest>,
    executions: Arc<RwLock<HashMap<ExecutionId, WorkExecution>>>,
}

#[async_trait]
impl WorkflowHandle for BasicWorkflowHandle {
    async fn submit(&self, request: ExecutionRequest) -> Result<ExecutionId, WorkflowError> {
        let id = next_exec_id();
        let execution = WorkExecution {
            id: id.clone(),
            work_id: request.work.id.clone(),
            workspace_id: request.work.workspace_id.clone(),
            status: WorkExecutionStatus::Pending,
            started_at: None,
            finished_at: None,
            tasks: request
                .work
                .tasks
                .iter()
                .map(|t| TaskExecution {
                    task_id: t.id.clone(),
                    status: TaskExecutionStatus::Pending,
                    started_at: None,
                    finished_at: None,
                    logs: vec![],
                    outputs: vec![],
                })
                .collect(),
        };
        self.executions
            .write()
            .await
            .insert(id.clone(), execution);
        self.tx
            .send(request)
            .await
            .map_err(|e| WorkflowError::Internal(format!("channel closed: {e}")))?;
        Ok(id)
    }

    async fn cancel(&self, id: &ExecutionId) -> Result<(), WorkflowError> {
        let mut g = self.executions.write().await;
        let exec = g
            .get_mut(id)
            .ok_or_else(|| WorkflowError::NotFound(id.0.clone()))?;
        match exec.status {
            WorkExecutionStatus::Pending | WorkExecutionStatus::Running => {
                exec.status = WorkExecutionStatus::Cancelled;
                Ok(())
            }
            _ => Err(WorkflowError::Internal(format!(
                "execution {} cannot be cancelled in state {:?}",
                id.0, exec.status
            ))),
        }
    }

    async fn status(&self, id: &ExecutionId) -> Result<WorkExecution, WorkflowError> {
        self.executions
            .read()
            .await
            .get(id)
            .cloned()
            .ok_or_else(|| WorkflowError::NotFound(id.0.clone()))
    }

    async fn list(
        &self,
        status_filter: Option<WorkExecutionStatus>,
    ) -> Result<Vec<WorkExecution>, WorkflowError> {
        let g = self.executions.read().await;
        Ok(match status_filter {
            None => g.values().cloned().collect(),
            Some(filter) => g.values().filter(|e| e.status == filter).cloned().collect(),
        })
    }
}

// ── Server ────────────────────────────────────────────────────────────────────

pub struct BasicWorkflowServer {
    handle: BasicWorkflowHandle,
    rx: mpsc::Receiver<ExecutionRequest>,
}

#[async_trait]
impl WorkflowServer for BasicWorkflowServer {
    fn name(&self) -> &str {
        "basic-workflow-server"
    }

    fn handle(&self) -> Arc<dyn WorkflowHandle> {
        Arc::new(self.handle.clone())
    }

    async fn run(mut self: Box<Self>) {
        info!("BasicWorkflowServer running");
        while let Some(request) = self.rx.recv().await {
            let executions = self.handle.executions.clone();
            // Spawn a task so the loop remains responsive to new submissions.
            tokio::spawn(async move {
                run_execution(executions, request).await;
            });
        }
        info!("BasicWorkflowServer channel closed — shutting down");
    }
}

/// Naive sequential execution: marks every task Running → Succeeded.
/// Replace with real DAG scheduling + executor dispatch in a future iteration.
async fn run_execution(
    executions: Arc<RwLock<HashMap<ExecutionId, WorkExecution>>>,
    request: ExecutionRequest,
) {
    // Find the matching execution by work_id (most recently submitted)
    let exec_id = {
        let g = executions.read().await;
        g.values()
            .filter(|e| e.work_id == request.work.id && e.status == WorkExecutionStatus::Pending)
            .map(|e| e.id.clone())
            .next()
    };

    let Some(exec_id) = exec_id else {
        warn!("No pending execution found for work {}", request.work.id.0);
        return;
    };

    // Mark as Running
    {
        let mut g = executions.write().await;
        if let Some(e) = g.get_mut(&exec_id) {
            if e.status == WorkExecutionStatus::Cancelled {
                return;
            }
            e.status = WorkExecutionStatus::Running;
            e.started_at = Some(now_str());
        }
    }

    // Run tasks sequentially (no real executor — just mark Succeeded)
    let task_ids: Vec<_> = request.work.tasks.iter().map(|t| t.id.clone()).collect();
    for task_id in task_ids {
        // Bail out if the work was cancelled
        let cancelled = executions
            .read()
            .await
            .get(&exec_id)
            .map(|e| e.status == WorkExecutionStatus::Cancelled)
            .unwrap_or(true);
        if cancelled {
            return;
        }

        let mut g = executions.write().await;
        if let Some(e) = g.get_mut(&exec_id) {
            if let Some(te) = e.tasks.iter_mut().find(|t| t.task_id == task_id) {
                te.status = TaskExecutionStatus::Running;
                te.started_at = Some(now_str());
            }
        }
        drop(g);

        // Yield to the executor — a real implementation would await actual work here.
        tokio::task::yield_now().await;

        let mut g = executions.write().await;
        if let Some(e) = g.get_mut(&exec_id) {
            if let Some(te) = e.tasks.iter_mut().find(|t| t.task_id == task_id) {
                te.status = TaskExecutionStatus::Succeeded;
                te.finished_at = Some(now_str());
            }
        }
    }

    // Mark work as Succeeded
    let mut g = executions.write().await;
    if let Some(e) = g.get_mut(&exec_id) {
        if e.status != WorkExecutionStatus::Cancelled {
            e.status = WorkExecutionStatus::Succeeded;
            e.finished_at = Some(now_str());
        }
    }
}

fn now_str() -> String {
    // RFC-3339-ish timestamp without pulling in chrono.
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    format!("{secs}")
}

// ── Factory ───────────────────────────────────────────────────────────────────

/// Channel buffer size for pending execution requests.
const CHANNEL_CAPACITY: usize = 256;

pub struct BasicWorkflowServerFactory;

impl WorkflowServerFactory for BasicWorkflowServerFactory {
    fn name(&self) -> &str {
        "basic-workflow-server"
    }

    fn build(&self, _config: Value) -> Result<Box<dyn WorkflowServer>, WorkflowError> {
        let (tx, rx) = mpsc::channel(CHANNEL_CAPACITY);
        let executions = Arc::new(RwLock::new(HashMap::new()));
        let handle = BasicWorkflowHandle { tx, executions };
        Ok(Box::new(BasicWorkflowServer { handle, rx }))
    }
}
