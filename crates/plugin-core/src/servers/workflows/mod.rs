//! Workflows server — manages Crons, schedules, and Workflow execution.
//!
//! # Configuration
//!
//! ```yaml
//! servers:
//!   workflows:
//!     component:
//!       plugin: orkester-plugin-core
//!       server: workflows-server
//!     enabled: true
//!     # REST server to register APIs on.
//!     rest_target: rest_api      # default: "rest_api"
//!     # How often the scheduler tick runs (seconds).
//!     scheduler_interval_seconds: 30  # default: 30
//! ```
//!
//! # Architecture
//!
//! ```
//!  ┌──────────────────────────────────────────────────┐
//!  │  WorkflowsServer  (hub participant)              │
//!  │                                                  │
//!  │  ┌─────────────────────┐  ┌──────────────────┐  │
//!  │  │  Scheduler loop     │  │  ApiHandler       │  │
//!  │  │  (fires Crons,      │  │  (REST via hub)   │  │
//!  │  │   creates Workflows)│  └──────────────────-┘  │
//!  │  └───────┬─────────────┘                         │
//!  │          │ tokio::spawn per Workflow              │
//!  │  ┌───────▼─────────────┐                         │
//!  │  │  Worker             │                         │
//!  │  │  (drives execution) │                         │
//!  │  └─────────────────────┘                         │
//!  │                                                  │
//!  │  ┌──────────────────────┐                        │
//!  │  │  WorkflowsStore      │                        │
//!  │  │  (PersistenceProvider│                        │
//!  │  └──────────────────────┘                        │
//!  └──────────────────────────────────────────────────┘
//! ```

pub mod api;
pub mod model;
pub mod store;
pub mod worker;
pub mod workspace_client;

use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use orkester_common::{log_error, log_info, log_warn};
use orkester_common::messaging::{Message, ServerSide};
use orkester_common::plugin::servers::{Server, ServerBuilder, ServerError};
use serde_json::{json, Value};

use api::ApiHandler;
use model::{Cron, ConcurrencyAction, Workflow, WorkflowStatus};
use store::WorkflowsStore;
use worker::{LocalWorker, Worker};
use workspace_client::WorkspaceClient;

use crate::persistence::MemoryPersistenceProvider;

// ── WorkflowsServer ────────────────────────────────────────────────────────────

pub struct WorkflowsServer {
    config: Value,
}

impl Server for WorkflowsServer {
    fn start(&self, channel: ServerSide) -> Result<(), ServerError> {
        let config = self.config.clone();

        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("Failed to build Tokio runtime");

            rt.block_on(run(config, channel));
        });

        Ok(())
    }

    fn stop(&self) -> Result<(), ServerError> {
        // The thread will stop when the hub drops the channel.
        Ok(())
    }
}

// ── Main async entry point ────────────────────────────────────────────────────

async fn run(config: Value, channel: ServerSide) {
    // ── Build persistence store ───────────────────────────────────────────
    let provider: Arc<dyn orkester_common::plugin::providers::persistence::PersistenceProvider> =
        Arc::new(MemoryPersistenceProvider::default());
    let store = WorkflowsStore::new(provider);

    // ── Config ────────────────────────────────────────────────────────────
    let rest_target = config
        .get("rest_target")
        .and_then(|v| v.as_str())
        .unwrap_or("rest_api")
        .to_string();

    let workspace_target = config
        .get("workspace_target")
        .and_then(|v| v.as_str())
        .unwrap_or("workspace")
        .to_string();

    let scheduler_interval = Duration::from_secs(
        config
            .get("scheduler_interval_seconds")
            .and_then(|v| v.as_u64())
            .unwrap_or(30),
    );

    // ── Register routes with the REST server ──────────────────────────────
    for (method, path) in api::ROUTES {
        let msg = Message::new(
            0,
            "",
            rest_target.as_str(),
            "register_route",
            json!({ "method": method, "path": path }),
        );
        if channel.to_hub.send(msg).is_err() {
            log_error!("Hub disconnected before routes could be registered.");
            return;
        }
    }
    log_info!("Route registrations sent to '{}'.", rest_target);

    // ── Build API handler ─────────────────────────────────────────────────
    let handler = ApiHandler {
        store: store.clone(),
        to_hub: channel.to_hub.clone(),
    };

    // ── Build workspace client ────────────────────────────────────────────
    let workspace_client = WorkspaceClient::new(workspace_target, channel.to_hub.clone());

    // ── Bridge std channel into Tokio ─────────────────────────────────────
    let from_hub = channel.from_hub;
    let (hub_tx, mut hub_rx) = tokio::sync::mpsc::unbounded_channel::<Message>();
    std::thread::spawn(move || {
        while let Ok(msg) = from_hub.recv() {
            if hub_tx.send(msg).is_err() {
                break;
            }
        }
    });

    // ── Scheduler tick ────────────────────────────────────────────────────
    let mut scheduler_tick = tokio::time::interval(scheduler_interval);

    // ── Event loop ────────────────────────────────────────────────────────
    log_info!("Server ready.");

    loop {
        tokio::select! {
            // Hub messages (HTTP requests + acks).
            Some(msg) = hub_rx.recv() => {
                match msg.message_type.as_str() {
                    "http_request" => {
                        handler.handle(msg).await;
                    }
                    "http_response" => {
                        workspace_client.handle_response(msg);
                    }
                    "route_registered" => {
                        let method = msg.content.get("method").and_then(|v| v.as_str()).unwrap_or("?");
                        let path   = msg.content.get("path").and_then(|v| v.as_str()).unwrap_or("?");
                        log_info!("Route confirmed: {} {}", method, path);
                    }
                    "error" => {
                        log_warn!(
                            "Hub error: {}",
                            msg.content.get("error").and_then(|v| v.as_str()).unwrap_or("unknown")
                        );
                    }
                    other => {
                        log_warn!("Unexpected message type '{}'", other);
                    }
                }
            }

            // Periodic scheduler tick.
            _ = scheduler_tick.tick() => {
                run_scheduler_tick(&store, &workspace_client).await;
            }

            else => break,
        }
    }

    log_info!("Server stopped.");
}

// ── Scheduler ─────────────────────────────────────────────────────────────────

/// On every tick: find enabled Crons whose `next_fire_at` is in the past and
/// apply the concurrency policy to decide whether to create a new Workflow.
async fn run_scheduler_tick(store: &WorkflowsStore, workspace: &WorkspaceClient) {
    let crons = match store.list_all_enabled_crons().await {
        Ok(c) => c,
        Err(e) => {
            log_error!("Scheduler: failed to list crons: {}", e);
            return;
        }
    };

    let now = Utc::now();

    for mut cron in crons {
        let should_fire = cron
            .next_fire_at
            .map(|t| t <= now)
            .unwrap_or(false);

        if !should_fire {
            continue;
        }

        log_info!(
            "Scheduler: cron '{}' fired (schedule: {})",
            cron.id,
            cron.schedule
        );

        // Find any active workflows for the same Work in the same namespace.
        let active = match store
            .list_active_workflows_for_work(
                &cron.namespace,
                &cron.work_name,
                &cron.work_version,
            )
            .await
        {
            Ok(v) => v,
            Err(e) => {
                log_error!(
                    "Scheduler: could not list active workflows for cron '{}': {}",
                    cron.id,
                    e
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
            let mut wf = Workflow::new(
                &cron.namespace,
                &cron.work_name,
                &cron.work_version,
            );
            wf.work_context = cron.work_context.clone();
            wf.execution = cron.execution.clone();
            wf.triggers.cron_id = Some(cron.id.clone());

            log_info!(
                "Scheduler: cron '{}' — creating workflow '{}'",
                cron.id, wf.id
            );

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

/// Compute the next fire time after `after` for a 5-field cron expression.
///
/// Delegated to [`Cron::next_fire_after`].
#[allow(dead_code)]
fn next_fire_after(
    schedule: &str,
    after: chrono::DateTime<Utc>,
) -> Option<chrono::DateTime<Utc>> {
    Cron::next_fire_after(schedule, after)
}

// ── Builder ────────────────────────────────────────────────────────────────────

pub struct WorkflowsServerBuilder;

impl ServerBuilder for WorkflowsServerBuilder {
    fn build(&self, config: Value) -> Result<Box<dyn Server>, ServerError> {
        Ok(Box::new(WorkflowsServer { config }))
    }
}
