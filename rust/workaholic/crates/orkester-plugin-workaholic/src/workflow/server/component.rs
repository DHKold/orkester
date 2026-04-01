//! WorkflowServerComponent — the main Orkester component for workflow execution.
//!
//! Responsibilities:
//! - Cron scheduling (register/unregister/list) with persistence.
//! - Manual work-run triggers forwarded to the background orchestrator.
//! - Query endpoints for WorkRun and TaskRun state.

use std::sync::Arc;

use orkester_plugin::prelude::*;
use serde_json::Value;
use workaholic::{CronDoc, TaskRunDoc, Trigger, WorkaholicError, WorkRunDoc};

use crate::workflow::{
    actions::*,
    cron::CronScheduler,
    request::{
        CronRefRequest, ListCronsResponse, ListTaskRunsResponse, ListWorkRunsResponse,
        TaskRunRefRequest, TriggerWorkRequest, WorkRunRefRequest,
    },
};

use super::{
    config::WorkflowServerConfig,
    orchestrator::{start_orchestrator, PendingTrigger},
    registry::WorkflowRegistry,
};
use crate::document::persistor::{LocalFsPersistor, MemoryPersistor};

// --- WorkflowServerComponent --------------------------------------------------

/// Orkester component that orchestrates workflow executions, manages cron
/// schedules, and exposes query endpoints for run state.
pub struct WorkflowServerComponent {
    registry:  Arc<WorkflowRegistry>,
    scheduler: CronScheduler,
    work_tx:   crossbeam_channel::Sender<PendingTrigger>,
}

unsafe impl Send for WorkflowServerComponent {}

#[component(
    kind        = "workaholic/WorkflowServer:1.0",
    name        = "Workflow Server",
    description = "Orchestrates workflow executions, manages crons, and handles manual triggers.",
)]
impl WorkflowServerComponent {
    pub fn new(host_ptr: *mut orkester_plugin::abi::AbiHost, config: WorkflowServerConfig) -> Self {
        let persistor = build_persistor(&config);
        let registry  = Arc::new(WorkflowRegistry::new(persistor));
        let (scheduler, fire_rx) = CronScheduler::start();

        scheduler.restore(registry.load_crons());

        let work_tx = start_orchestrator(
            host_ptr as usize,
            config.catalog_ref.clone(),
            config.namespace.clone(),
            Arc::clone(&registry),
        );

        start_cron_forwarder(fire_rx, work_tx.clone());

        Self { registry, scheduler, work_tx }
    }

    // -- Cron management ----------------------------------------------------

    #[handle(ACTION_WORKFLOW_REGISTER_CRON)]
    fn register_cron(&mut self, cron: CronDoc) -> Result<(), WorkaholicError> {
        log_info!("[workflow-server] registering cron '{}'", cron.name);
        self.registry.upsert_cron(&cron);
        self.scheduler.register(cron);
        Ok(())
    }

    #[handle(ACTION_WORKFLOW_UNREGISTER_CRON)]
    fn unregister_cron(&mut self, req: CronRefRequest) -> Result<(), WorkaholicError> {
        log_info!("[workflow-server] unregistering cron '{}'", req.name);
        self.scheduler.unregister(req.name.clone());
        self.registry.remove_cron(&req.name);
        Ok(())
    }

    #[handle(ACTION_WORKFLOW_LIST_CRONS)]
    fn list_crons(&mut self, _: Value) -> Result<ListCronsResponse, WorkaholicError> {
        Ok(ListCronsResponse { crons: self.scheduler.list_crons() })
    }

    // -- Trigger ------------------------------------------------------------

    /// Accept a trigger and hand it off to the background orchestrator thread.
    ///
    /// Must NOT call `host.handle()` here — doing so would deadlock the HUB
    /// pipeline worker.
    #[handle(ACTION_WORKFLOW_TRIGGER)]
    fn trigger_work(&mut self, req: TriggerWorkRequest) -> Result<Value, WorkaholicError> {
        log_info!("[workflow-server] trigger received: work_ref='{}'", req.work_ref);
        self.work_tx.send(PendingTrigger {
            work_ref: req.work_ref,
            trigger: Trigger {
                trigger_type: "manual".into(),
                at:           Some(chrono::Utc::now().to_rfc3339()),
                identity:     Some("manual".into()),
            },
            inputs: req.inputs,
        }).ok();
        Ok(serde_json::json!({ "status": "accepted" }))
    }

    // -- WorkRun queries ----------------------------------------------------

    #[handle(ACTION_WORKFLOW_LIST_WORK_RUNS)]
    fn list_work_runs(&mut self, _: Value) -> Result<ListWorkRunsResponse, WorkaholicError> {
        Ok(ListWorkRunsResponse { work_runs: self.registry.list_work_runs() })
    }

    #[handle(ACTION_WORKFLOW_GET_WORK_RUN)]
    fn get_work_run(&mut self, req: WorkRunRefRequest) -> Result<WorkRunDoc, WorkaholicError> {
        self.registry.get_work_run(&req.name)
            .ok_or_else(|| WorkaholicError::NotFound { kind: "WorkRun".into(), name: req.name })
    }

    #[handle(ACTION_WORKFLOW_CANCEL_WORK_RUN)]
    fn cancel_work_run(&mut self, req: WorkRunRefRequest) -> Result<(), WorkaholicError> {
        log_warn!("[workflow-server] cancel requested for '{}' (not yet implemented)", req.name);
        Ok(())
    }

    // -- TaskRun queries ----------------------------------------------------

    #[handle(ACTION_WORKFLOW_LIST_TASK_RUNS)]
    fn list_task_runs(&mut self, _: Value) -> Result<ListTaskRunsResponse, WorkaholicError> {
        Ok(ListTaskRunsResponse { task_runs: self.registry.list_task_runs() })
    }

    #[handle(ACTION_WORKFLOW_GET_TASK_RUN)]
    fn get_task_run(&mut self, req: TaskRunRefRequest) -> Result<TaskRunDoc, WorkaholicError> {
        self.registry.get_task_run(&req.name)
            .ok_or_else(|| WorkaholicError::NotFound { kind: "TaskRun".into(), name: req.name })
    }
}

// ── Private helpers ───────────────────────────────────────────────────────────

/// Build the `DocumentPersistor` from server config.
fn build_persistor(
    config: &WorkflowServerConfig,
) -> Arc<dyn workaholic::DocumentPersistor> {
    match &config.persist_path {
        Some(path) => {
            let _ = std::fs::create_dir_all(path);
            log_info!("[workflow-server] using local-fs persistor at '{path}'");
            Arc::new(LocalFsPersistor::new(path.clone()))
        }
        None => {
            log_info!("[workflow-server] using in-memory persistor (state will not survive restart)");
            Arc::new(MemoryPersistor::new())
        }
    }
}

/// Spawn a thread that forwards cron-fire events to the orchestrator channel.
fn start_cron_forwarder(
    fire_rx: crossbeam_channel::Receiver<(CronDoc, workaholic::Trigger)>,
    work_tx: crossbeam_channel::Sender<PendingTrigger>,
) {
    std::thread::spawn(move || {
        for (cron, trigger) in fire_rx {
            log_info!("[cron] fired '{}' → triggering work_ref='{}'", cron.name, cron.spec.work_ref);
            work_tx.send(PendingTrigger {
                work_ref: cron.spec.work_ref.clone(),
                trigger,
                inputs: Default::default(),
            }).ok();
        }
    });
}