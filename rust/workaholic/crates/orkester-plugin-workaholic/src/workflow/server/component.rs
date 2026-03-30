//! WorkflowServerComponent - the main Orkester component for workflow execution.

use std::sync::Arc;

use orkester_plugin::prelude::*;
use serde_json::Value;
use workaholic::{CronDoc, TaskRunDoc, Trigger, WorkaholicError, WorkRunDoc};

use crate::workflow::{
    actions::*,
    cron::CronScheduler,
    request::{CronRefRequest, ListCronsResponse, ListTaskRunsResponse, ListWorkRunsResponse, TaskRunRefRequest, TriggerWorkRequest, WorkRunRefRequest},
};

use super::{
    config::WorkflowServerConfig,
    orchestrator::{start_orchestrator, PendingTrigger},
    registry::WorkflowRegistry,
};

// --- WorkflowServerComponent --------------------------------------------------

pub struct WorkflowServerComponent {
    registry:  Arc<WorkflowRegistry>,
    scheduler: CronScheduler,
    work_tx:   crossbeam_channel::Sender<PendingTrigger>,
    config:    WorkflowServerConfig,
}

unsafe impl Send for WorkflowServerComponent {}

#[component(
    kind        = "workaholic/WorkflowServer:1.0",
    name        = "Workflow Server",
    description = "Orchestrates workflow executions, manages crons, and handles manual triggers.",
)]
impl WorkflowServerComponent {
    pub fn new(host_ptr: *mut orkester_plugin::abi::AbiHost, config: WorkflowServerConfig) -> Self {
        let registry = Arc::new(WorkflowRegistry::new());
        let (scheduler, fire_rx) = CronScheduler::start();
        let host_raw = host_ptr as usize;

        let work_tx = start_orchestrator(
            host_raw,
            config.catalog_ref.clone(),
            config.namespace.clone(),
            Arc::clone(&registry),
        );

        // Forward cron fires to the orchestrator as regular work triggers.
        let work_tx_cron = work_tx.clone();
        std::thread::spawn(move || {
            for (cron, trigger) in fire_rx {
                eprintln!("[cron] fired: {} -> triggering '{}'", cron.name, cron.spec.work_ref);
                work_tx_cron.send(PendingTrigger {
                    work_ref: cron.spec.work_ref.clone(),
                    trigger,
                    inputs: Default::default(),
                }).ok();
            }
        });

        Self { registry, scheduler, work_tx, config }
    }

    // -- Cron management ----------------------------------------------------

    #[handle(ACTION_WORKFLOW_REGISTER_CRON)]
    fn register_cron(&mut self, cron: CronDoc) -> Result<(), WorkaholicError> {
        self.scheduler.register(cron);
        Ok(())
    }

    #[handle(ACTION_WORKFLOW_UNREGISTER_CRON)]
    fn unregister_cron(&mut self, req: CronRefRequest) -> Result<(), WorkaholicError> {
        self.scheduler.unregister(req.name);
        Ok(())
    }

    #[handle(ACTION_WORKFLOW_LIST_CRONS)]
    fn list_crons(&mut self, _: Value) -> Result<ListCronsResponse, WorkaholicError> {
        Ok(ListCronsResponse { crons: self.scheduler.list_crons() })
    }

    // -- Trigger ------------------------------------------------------------

    /// Accept a trigger and hand it off to the background orchestrator thread.
    ///
    /// IMPORTANT: must NOT call `host.handle()` here -- doing so would deadlock
    /// the pipeline (the worker thread cannot process a new request while it
    /// is already inside this handler).
    #[handle(ACTION_WORKFLOW_TRIGGER)]
    fn trigger_work(&mut self, req: TriggerWorkRequest) -> Result<Value, WorkaholicError> {
        eprintln!("[workflow-server] trigger received: work_ref='{}'", req.work_ref);
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
        eprintln!("[workflow-server] cancel requested for '{}'", req.name);
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