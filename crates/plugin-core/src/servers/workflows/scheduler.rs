//! Scheduler — fires enabled Crons on their schedule and spawns Worker tasks.

use chrono::Utc;
use orkester_common::{log_error, log_info};

use super::model::{Cron, ConcurrencyAction, Workflow, WorkflowStatus};
use super::store::WorkflowsStore;
use super::worker::{LocalWorker, Worker};
use super::workspace_client::WorkspaceClient;

/// On every tick: find enabled Crons whose `next_fire_at` is in the past and
/// apply the concurrency policy to decide whether to create a new Workflow.
pub async fn run_tick(store: &WorkflowsStore, workspace: &WorkspaceClient) {
    let crons = match store.list_all_enabled_crons().await {
        Ok(c) => c,
        Err(e) => {
            log_error!("Scheduler: failed to list crons: {}", e);
            return;
        }
    };

    let now = Utc::now();

    for mut cron in crons {
        let should_fire = cron.next_fire_at.map(|t| t <= now).unwrap_or(false);

        if !should_fire {
            continue;
        }

        log_info!("Scheduler: cron '{}' fired (schedule: {})", cron.id, cron.schedule);

        // Find any active workflows for the same Work in the same namespace.
        let active = match store
            .list_active_workflows_for_work(&cron.namespace, &cron.work_name, &cron.work_version)
            .await
        {
            Ok(v) => v,
            Err(e) => {
                log_error!(
                    "Scheduler: could not list active workflows for cron '{}': {}",
                    cron.id, e
                );
                continue;
            }
        };

        let mut create_new = true;

        for existing in &active {
            let action = match existing.status {
                WorkflowStatus::Waiting => &cron.concurrency_policy.on_waiting,
                WorkflowStatus::Running => &cron.concurrency_policy.on_running,
                WorkflowStatus::Paused  => &cron.concurrency_policy.on_paused,
                _                       => &cron.concurrency_policy.default_action,
            };

            match action {
                ConcurrencyAction::Skip => {
                    log_info!(
                        "Scheduler: cron '{}' — skipping (existing workflow '{}' is {})",
                        cron.id, existing.id, existing.status
                    );
                    create_new = false;
                    break;
                }
                ConcurrencyAction::CancelExisting => {
                    log_info!(
                        "Scheduler: cron '{}' — cancelling existing workflow '{}' and skipping",
                        cron.id, existing.id
                    );
                    cancel_workflow(store, &existing.namespace, &existing.id).await;
                    create_new = false;
                }
                ConcurrencyAction::Replace => {
                    log_info!(
                        "Scheduler: cron '{}' — replacing existing workflow '{}'",
                        cron.id, existing.id
                    );
                    cancel_workflow(store, &existing.namespace, &existing.id).await;
                    // create_new stays true — a new one will be spawned below.
                }
                ConcurrencyAction::Allow => {
                    // Do nothing; create_new stays true.
                }
            }
        }

        if create_new {
            let mut wf = Workflow::new(&cron.namespace, &cron.work_name, &cron.work_version);
            wf.work_context = cron.work_context.clone();
            wf.execution = cron.execution.clone();
            wf.triggers.cron_id = Some(cron.id.clone());

            log_info!("Scheduler: cron '{}' — creating workflow '{}'", cron.id, wf.id);

            match store.put_workflow(&wf).await {
                Ok(()) => {
                    let store_clone = store.clone();
                    let wf_clone = wf.clone();
                    let workspace_clone = workspace.clone();
                    tokio::spawn(async move {
                        LocalWorker.run(wf_clone, store_clone, workspace_clone).await;
                    });
                }
                Err(e) => {
                    log_error!(
                        "Scheduler: failed to persist workflow for cron '{}': {}",
                        cron.id, e
                    );
                }
            }
        }

        // Advance next_fire_at.
        cron.last_fired_at = Some(now);
        cron.next_fire_at = Cron::next_fire_after(&cron.schedule, now);
        cron.updated_at = now;
        if let Err(e) = store.put_cron(&cron).await {
            log_error!(
                "Scheduler: failed to update cron '{}' after firing: {}",
                cron.id, e
            );
        }
    }
}

/// Cancel a workflow by marking it as Cancelled in the store.
async fn cancel_workflow(store: &WorkflowsStore, namespace: &str, id: &str) {
    match store.get_workflow(namespace, id).await {
        Ok(mut wf) => {
            wf.status = WorkflowStatus::Cancelled;
            wf.finished_at = Some(Utc::now());
            wf.updated_at = Utc::now();
            if let Err(e) = store.put_workflow(&wf).await {
                log_error!("Scheduler: failed to cancel workflow '{}': {}", id, e);
            }
        }
        Err(e) => log_error!("Scheduler: workflow '{}' not found for cancel: {}", id, e),
    }
}
