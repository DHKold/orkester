//! Background trigger orchestrator.
//!
//! Receives `PendingTrigger` messages from the component handler (which returns
//! immediately) and performs the actual catalog lookup + task execution off the
//! pipeline worker thread, avoiding the re-entrancy deadlock.

use std::collections::HashMap;
use std::sync::Arc;

use crossbeam_channel::Sender;
use workaholic::{
    DocumentMetadata, TaskRunRequestDoc, Trigger, WorkRunDoc, WorkRunRequestDoc,
    WorkRunSpec, WorkRunState, WorkRunStatus, WorkRunStepStatus, WorkRunSummary,
    WorkaholicError, WORK_RUN_KIND,
};

use crate::workflow::trigger::resolver::{ResolutionInput, TriggerResolver};

use super::{
    catalog_client::CatalogClient,
    executor::execute_work_run,
    registry::WorkflowRegistry,
};

// ─── Message type ─────────────────────────────────────────────────────────────

pub struct PendingTrigger {
    pub work_ref: String,
    pub trigger:  Trigger,
    pub inputs:   HashMap<String, serde_json::Value>,
}

// ─── Thread spawner ───────────────────────────────────────────────────────────

/// Spawn the background orchestrator thread and return its send channel.
pub fn start_orchestrator(
    host_raw:    usize,
    catalog_ref: String,
    namespace:   String,
    registry:    Arc<WorkflowRegistry>,
) -> Sender<PendingTrigger> {
    let (tx, rx) = crossbeam_channel::unbounded::<PendingTrigger>();

    std::thread::spawn(move || {
        for pending in rx {
            eprintln!("[orchestrator] trigger received for '{}'", pending.work_ref);
            let host = unsafe { orkester_plugin::sdk::Host::from_abi(host_raw as *mut _) };
            let mut client = CatalogClient::new(host, &catalog_ref);
            if let Err(e) = do_trigger(&mut client, pending, &namespace, &registry) {
                eprintln!("[orchestrator] trigger failed: {e}");
            }
        }
    });

    tx
}

// ─── Trigger handler (runs on orchestrator thread) ────────────────────────────

fn do_trigger(
    client:    &mut CatalogClient,
    pending:   PendingTrigger,
    namespace: &str,
    registry:  &WorkflowRegistry,
) -> Result<(), WorkaholicError> {
    eprintln!("[orchestrator] loading work '{}'...", pending.work_ref);
    let work  = client.get_work(&pending.work_ref)?;
    let tasks = client.get_tasks_by_ref(namespace)?;
    eprintln!("[orchestrator] loaded {} task(s) in namespace '{namespace}'", tasks.len());

    let out = TriggerResolver::resolve(ResolutionInput {
        work:            &work,
        tasks:           &tasks,
        trigger:         pending.trigger,
        input_overrides: pending.inputs,
        work_runner_ref: "orchestrator".into(),
    });

    let run_name = out.work_run_request.name.clone();
    registry.insert_work_request(out.work_run_request.clone());
    registry.insert_task_requests(out.task_run_requests.clone());
    registry.insert_work_run(make_run_doc(&out.work_run_request));

    let task_map: HashMap<String, TaskRunRequestDoc> = out
        .task_run_requests
        .into_iter()
        .map(|r| (r.name.clone(), r))
        .collect();

    eprintln!("[orchestrator] executing run '{run_name}' ({} step(s))...", task_map.len());
    execute_work_run(&out.work_run_request, &task_map, registry);
    eprintln!("[orchestrator] run '{run_name}' finished");
    Ok(())
}

// ─── Doc builder ──────────────────────────────────────────────────────────────

fn make_run_doc(req: &WorkRunRequestDoc) -> WorkRunDoc {
    let now = chrono::Utc::now().to_rfc3339();
    let n   = req.spec.steps.len();
    WorkRunDoc {
        kind:    WORK_RUN_KIND.into(),
        name:    req.name.clone(),
        version: "1.0.0".into(),
        metadata: DocumentMetadata {
            namespace:   req.metadata.namespace.clone(),
            owner:       None,
            description: None,
            tags:        vec![],
            extra:       Default::default(),
        },
        spec: WorkRunSpec {
            work_run_request_ref: req.name.clone(),
            work_ref:             req.spec.work_ref.clone(),
            work_runner_ref:      "orchestrator".into(),
            trigger:              req.spec.trigger.clone(),
        },
        status: Some(WorkRunStatus {
            state:        WorkRunState::Running,
            created_at:   Some(now.clone()),
            started_at:   Some(now),
            finished_at:  None,
            summary: WorkRunSummary {
                total_steps:     n,
                pending_steps:   n,
                running_steps:   0,
                succeeded_steps: 0,
                failed_steps:    0,
                cancelled_steps: 0,
            },
            steps: req.spec.steps.iter().map(|s| WorkRunStepStatus {
                name:                  s.name.clone(),
                state:                 WorkRunState::Pending,
                task_run_request_ref:  Some(s.task_run_request_ref.clone()),
                active_task_run_ref:   None,
                attempts:              0,
            }).collect(),
            outputs:       Default::default(),
            state_history: vec![],
        }),
    }
}
