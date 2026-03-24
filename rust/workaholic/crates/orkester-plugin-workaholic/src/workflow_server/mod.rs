pub mod config;
pub mod dag;
pub mod state;

use std::sync::Arc;

use chrono::Utc;
use orkester_plugin::{prelude::*, sdk::Host};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use workaholic::{
    execution::{
        work_run::{TriggerKind, WorkRunPhase, WorkRunSpec, WorkRunStatus, WorkRunTrigger},
        WorkRun,
    },
    traits,
};

use crate::{
    host_client::HostClient,
    persistence_server::PersistenceClient,
    worker::{WorkerContext, WorkerHandle, thread::ThreadWorker},
};

use config::WorkflowServerConfig;
use state::WorkflowServerState;

// collection name constants
use crate::worker::thread::{TASK_RUNS, WORK_RUNS};

// ── WorkflowServer ────────────────────────────────────────────────────────────

pub struct WorkflowServer {
    state: WorkflowServerState,
}

impl WorkflowServer {
    pub fn new(config: WorkflowServerConfig, host: Host) -> Self {
        let host_client = HostClient::new(host);

        // Persistence is provided by a separately-registered component.
        // `PersistenceClient` forwards all `persistence/*` calls through the
        // host to the component with the configured name.
        let persistence = Arc::new(
            PersistenceClient::new(host_client.clone(), &config.persistence)
        ) as Arc<dyn workaholic::traits::PersistenceProvider>;

        host_client.log("info", "workflow",
            &format!("using persistence component '{}'", config.persistence));

        // Spawn inline workers.
        let mut workers: Vec<WorkerHandle> = Vec::new();
        for wcfg in &config.workers {
            let ctx = WorkerContext {
                host:        host_client.clone(),
                persistence: persistence.clone(),
                worker_name: wcfg.name.clone(),
            };
            let handle = ThreadWorker::spawn(wcfg, ctx);
            host_client.log("info", "workflow",
                &format!("spawned worker '{}' (kind={})", wcfg.name, wcfg.kind));
            workers.push(handle);
        }

        // Recover in-progress work runs and requeue them.
        if let Ok(active) = traits::retrieve_all::<WorkRun>(&*persistence, WORK_RUNS) {
            let active: Vec<_> = active.into_iter()
                .filter(|r| r.status.as_ref().map(|s| !s.phase.is_terminal()).unwrap_or(true))
                .collect();
            if !active.is_empty() {
                host_client.log("info", "workflow",
                    &format!("{} active work run(s) found on startup (recovery pending)", active.len()));
            }
        }

        let state = WorkflowServerState {
            persistence,
            host: host_client,
            default_namespace: config.default_namespace,
            workers,
            next_worker: 0,
        };

        Self { state }
    }
}

// ── Request / response message types ─────────────────────────────────────────

#[derive(Deserialize)]
pub struct CreateWorkRunRequest {
    pub work_ref: String,
    #[serde(default)]
    pub namespace: Option<String>,
    #[serde(default)]
    pub trigger: Option<TriggerKind>,
    #[serde(default)]
    pub trigger_ref: Option<String>,
    #[serde(default)]
    pub params: serde_json::Value,
    /// If true (default), immediately queue the work run for execution after
    /// creating it.  Set to false for deferred / externally triggered runs.
    #[serde(default = "default_true")]
    pub auto_queue: bool,
}

fn default_true() -> bool { true }

#[derive(Deserialize)]
pub struct QueueWorkRunRequest {
    pub work_run_id: String,
}

#[derive(Deserialize)]
pub struct CancelWorkRunRequest {
    pub work_run_id: String,
    #[serde(default)]
    pub force: bool,
}

#[derive(Deserialize)]
pub struct GetWorkRunRequest {
    pub id: String,
}

#[derive(Deserialize)]
pub struct ListWorkRunsRequest {
    #[serde(default)]
    pub namespace: Option<String>,
    #[serde(default)]
    pub work_ref: Option<String>,
    #[serde(default)]
    pub phase: Option<WorkRunPhase>,
}

#[derive(Deserialize)]
pub struct GetTaskRunRequest {
    pub id: String,
}

#[derive(Deserialize)]
pub struct ListTaskRunsRequest {
    pub work_run_id: String,
}

#[derive(Deserialize)]
pub struct UpdateTaskRunStateRequest {
    pub task_run_id: String,
    pub phase: workaholic::execution::task_run::TaskRunPhase,
    #[serde(default)]
    pub outputs: serde_json::Value,
    #[serde(default)]
    pub external_id: Option<String>,
    #[serde(default)]
    pub error: Option<workaholic::execution::task_run::TaskRunError>,
}

#[derive(Serialize)]
pub struct WorkerSummary {
    pub name: String,
    pub kind: String,
    pub active_work_runs: usize,
    pub capacity: usize,
}

// ── PluginComponent impl (via macro) ─────────────────────────────────────────

#[component(
    kind        = "workaholic/WorkflowServer:1.0",
    name        = "WorkflowServer",
    description = "Manages workflow execution: workers, work runs, and task runs."
)]
impl WorkflowServer {
    // ── WorkRun lifecycle ────────────────────────────────────────────────────

    /// Create a new WorkRun and persist it in Pending state.
    #[handle("workflow/CreateWorkRun")]
    fn create_work_run(&mut self, req: CreateWorkRunRequest) -> Result<WorkRun> {
        let namespace = req.namespace.as_deref().unwrap_or(&self.state.default_namespace).to_string();
        let id = Uuid::new_v4().to_string();

        let mut work_run = WorkRun {
            kind: "orkester/workrun:1.0".into(),
            name: id.clone(),
            namespace,
            version: "1.0.0".into(),
            metadata: Default::default(),
            spec: WorkRunSpec {
                work_ref: req.work_ref,
                trigger: WorkRunTrigger {
                    kind: req.trigger.unwrap_or(TriggerKind::Api),
                    reference: req.trigger_ref,
                },
                params: req.params,
            },
            status: Some(WorkRunStatus { phase: WorkRunPhase::Pending, ..Default::default() }),
        };

        traits::persist(&*self.state.persistence, WORK_RUNS, &id, &work_run)
            .map_err(|e| -> Error { format!("failed to persist work run: {e}").into() })?;

        let work_ref = work_run.spec.work_ref.clone();
        self.state.host.log("info", "workflow",
            &format!("work run '{id}' created (work_ref={work_ref})"));

        // Auto-queue unless explicitly disabled.
        if req.auto_queue {
            work_run.status.get_or_insert_with(WorkRunStatus::default).phase = WorkRunPhase::Ready;
            traits::persist(&*self.state.persistence, WORK_RUNS, &id, &work_run)
                .map_err(|e| -> Error { format!("failed to persist queued state: {e}").into() })?;

            let worker = self.state.pick_worker()
                .ok_or_else(|| -> Error { "no workers available to queue work run".into() })?;
            let worker_name = worker.name.clone();
            worker.enqueue(id.clone()).map_err(|e| -> Error { e.into() })?;

            self.state.host.log("info", "workflow",
                &format!("work run '{id}' queued to worker '{worker_name}'"));
        }

        Ok(work_run)
    }

    /// Enqueue a previously created WorkRun for execution.
    #[handle("workflow/QueueWorkRun")]
    fn queue_work_run(&mut self, req: QueueWorkRunRequest) -> Result<WorkRun> {
        let id = &req.work_run_id;
        let mut run: WorkRun = traits::retrieve(&*self.state.persistence, WORK_RUNS, id)
            .map_err(|e| -> Error { format!("persistence error: {e}").into() })?
            .ok_or_else(|| -> Error { format!("work run '{id}' not found").into() })?;

        let phase = run.status.as_ref().map(|s| s.phase.clone()).unwrap_or_default();
        if phase != WorkRunPhase::Pending && phase != WorkRunPhase::Ready {
            return Err(format!("work run '{id}' is in phase {phase:?}, cannot queue").into());
        }

        run.status.get_or_insert_with(WorkRunStatus::default).phase = WorkRunPhase::Ready;
        traits::persist(&*self.state.persistence, WORK_RUNS, id, &run)
            .map_err(|e| -> Error { format!("persist error: {e}").into() })?;

        let worker = self.state.pick_worker()
            .ok_or_else(|| -> Error { "no workers available".into() })?;

        worker.enqueue(id.clone()).map_err(|e| -> Error { e.into() })?;

        self.state.host.log("info", "workflow", &format!("work run '{id}' queued"));
        Ok(run)
    }

    /// Cancel a work run that is Pending, Ready, or Running.
    #[handle("workflow/CancelWorkRun")]
    fn cancel_work_run(&mut self, req: CancelWorkRunRequest) -> Result<WorkRun> {
        let id = &req.work_run_id;
        let mut run: WorkRun = traits::retrieve(&*self.state.persistence, WORK_RUNS, id)
            .map_err(|e| -> Error { format!("persistence error: {e}").into() })?
            .ok_or_else(|| -> Error { format!("work run '{id}' not found").into() })?;

        let status = run.status.get_or_insert_with(WorkRunStatus::default);
        if status.phase.is_terminal() {
            return Err(format!("work run '{id}' is already in terminal phase {:?}", status.phase).into());
        }
        status.phase = WorkRunPhase::Cancelled;
        status.finished_at = Some(Utc::now());

        traits::persist(&*self.state.persistence, WORK_RUNS, id, &run)
            .map_err(|e| -> Error { format!("persist error: {e}").into() })?;

        self.state.host.log("info", "workflow",
            &format!("work run '{id}' cancelled (force={})", req.force));
        Ok(run)
    }

    /// Retrieve a single WorkRun by ID.
    #[handle("workflow/GetWorkRun")]
    fn get_work_run(&mut self, req: GetWorkRunRequest) -> Result<WorkRun> {
        traits::retrieve(&*self.state.persistence, WORK_RUNS, &req.id)
            .map_err(|e| -> Error { format!("persistence error: {e}").into() })?
            .ok_or_else(|| -> Error { format!("work run '{}' not found", req.id).into() })
    }

    /// List work runs with optional filters.
    #[handle("workflow/ListWorkRuns")]
    fn list_work_runs(&mut self, req: ListWorkRunsRequest) -> Result<Vec<WorkRun>> {
        let all: Vec<WorkRun> = traits::retrieve_all(&*self.state.persistence, WORK_RUNS)
            .map_err(|e| -> Error { format!("persistence error: {e}").into() })?;

        Ok(all.into_iter().filter(|r| {
            if let Some(ns) = &req.namespace { if &r.namespace != ns { return false; } }
            if let Some(wr) = &req.work_ref { if &r.spec.work_ref != wr { return false; } }
            if let Some(ph) = &req.phase { if r.status.as_ref().map(|s| &s.phase) != Some(ph) { return false; } }
            true
        }).collect())
    }

    // ── TaskRun operations ───────────────────────────────────────────────────

    /// Retrieve a single TaskRun by ID.
    #[handle("workflow/GetTaskRun")]
    fn get_task_run(&mut self, req: GetTaskRunRequest) -> Result<workaholic::execution::TaskRun> {
        traits::retrieve(&*self.state.persistence, TASK_RUNS, &req.id)
            .map_err(|e| -> Error { format!("persistence error: {e}").into() })?
            .ok_or_else(|| -> Error { format!("task run '{}' not found", req.id).into() })
    }

    /// List all task runs for a given work run.
    #[handle("workflow/ListTaskRuns")]
    fn list_task_runs(&mut self, req: ListTaskRunsRequest) -> Result<Vec<workaholic::execution::TaskRun>> {
        let all: Vec<workaholic::execution::TaskRun> =
            traits::retrieve_all(&*self.state.persistence, TASK_RUNS)
            .map_err(|e| -> Error { format!("persistence error: {e}").into() })?;

        Ok(all.into_iter()
            .filter(|r| r.spec.work_run_ref == req.work_run_id)
            .collect())
    }

    /// External update of a task run's state (used by remote task runners).
    #[handle("workflow/UpdateTaskRunState")]
    fn update_task_run_state(&mut self, req: UpdateTaskRunStateRequest) -> Result<workaholic::execution::TaskRun> {
        let mut task_run: workaholic::execution::TaskRun =
            traits::retrieve(&*self.state.persistence, TASK_RUNS, &req.task_run_id)
            .map_err(|e| -> Error { format!("persistence error: {e}").into() })?
            .ok_or_else(|| -> Error { format!("task run '{}' not found", req.task_run_id).into() })?;

        let status = task_run.status.get_or_insert_with(workaholic::execution::task_run::TaskRunStatus::default);
        status.phase = req.phase;
        status.outputs = req.outputs;
        if let Some(eid) = req.external_id { status.external_id = Some(eid); }
        if let Some(err) = req.error { status.error = Some(err); }
        if status.phase.is_terminal() { status.finished_at = Some(Utc::now()); }

        traits::persist(&*self.state.persistence, TASK_RUNS, &req.task_run_id, &task_run)
            .map_err(|e| -> Error { format!("persist error: {e}").into() })?;
        Ok(task_run)
    }

    // ── Worker management ────────────────────────────────────────────────────

    #[handle("workflow/ListWorkers")]
    fn list_workers(&mut self, _req: serde_json::Value) -> Result<Vec<WorkerSummary>> {
        Ok(self.state.workers.iter().map(|w| WorkerSummary {
            name: w.name.clone(),
            kind: w.kind.clone(),
            active_work_runs: w.active(),
            capacity: w.max_work_runs,
        }).collect())
    }

    // ── Recovery ─────────────────────────────────────────────────────────────

    #[handle("workflow/RecoverWorkRuns")]
    fn recover_work_runs(&mut self, _req: serde_json::Value) -> Result<usize> {
        let all: Vec<WorkRun> = traits::retrieve_all(&*self.state.persistence, WORK_RUNS)
            .map_err(|e| -> Error { format!("persist error: {e}").into() })?;

        let active: Vec<_> = all.into_iter()
            .filter(|r| r.status.as_ref().map(|s| !s.phase.is_terminal()).unwrap_or(true))
            .collect();

        let mut requeued = 0usize;
        for run in &active {
            let id = run.name.clone();
            let phase = run.status.as_ref().map(|s| s.phase.clone()).unwrap_or_default();
            if phase == WorkRunPhase::Running || phase == WorkRunPhase::Ready {
                if let Some(worker) = self.state.pick_worker() {
                    if let Err(e) = worker.enqueue(id.clone()) {
                        self.state.host.log("warn", "workflow",
                            &format!("failed to requeue work run '{id}': {e}"));
                    } else {
                        requeued += 1;
                    }
                }
            }
        }

        self.state.host.log("info", "workflow",
            &format!("recovery: requeued {requeued}/{} active work runs", active.len()));
        Ok(requeued)
    }
}
