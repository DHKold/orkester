//! Build, start, and stop server instances.

use std::sync::Arc;

use crate::messaging::{self, HubSide};
use crate::registry::DynamicRegistry;
use orkester_common::plugin::servers::{Server, ServerContext};
use orkester_common::plugin::Registry;
use orkester_common::{log_debug, log_error, log_info, log_warn};

use super::config::ServerEntry;

// ── RunningServer ─────────────────────────────────────────────────────────────

/// A server that has been built and started.
///
/// **Drop order**: `server` before `channel` — the server is stopped before
/// the channel (and its underlying pipe) is torn down.
pub struct RunningServer {
    /// Instance name as declared in config (e.g. `"rest_api"`).
    pub instance_name: String,
    /// Registry key used to find the builder (e.g. `"orkester-plugin-core:axum-rest-server"`).
    pub component_key: String,
    server: Box<dyn Server>,
}

// ── Start ─────────────────────────────────────────────────────────────────────

/// Build and start every entry using the registered builders.
///
/// Returns `(running_servers, hub_sides)` — the caller must register every
/// `HubSide` with the [`Hub`](crate::messaging::Hub) before the main loop.
///
/// Entries for which no matching builder is found are logged as errors and
/// skipped — they do **not** abort startup of subsequent servers.
pub fn start(entries: &[ServerEntry], registry: &Arc<DynamicRegistry>) -> (Vec<RunningServer>, Vec<HubSide>) {
    let mut running = Vec::with_capacity(entries.len());
    let mut hub_sides = Vec::with_capacity(entries.len());

    // Build the executor registry once; every server shares the same Arc.
    let executor_registry = registry.build_executor_registry();

    for entry in entries {
        let component_key = format!("{}:{}", entry.plugin_id, entry.server_id);

        log_info!(
            "Starting server '{}' (component='{}')...",
            entry.instance_name,
            component_key
        );

        let builder = match registry.server_builder(&entry.server_id) {
            Ok(orkester_common::plugin::PluginComponent::Server(b)) => b,
            Ok(_) => {
                log_error!(
                    "Component '{}' is not a Server builder — skipping.",
                    component_key
                );
                continue;
            }
            Err(e) => {
                log_error!("No builder registered for '{}' (instance '{}'). Is the plugin providing it loaded? ({})", component_key, entry.instance_name, e);
                continue;
            }
        };

        log_debug!(
            "Building server '{}' with config: {}",
            entry.instance_name,
            entry.config
        );

        let server = match builder.build(entry.config.clone()) {
            Ok(s) => s,
            Err(e) => {
                log_error!("Builder failed for server '{}': {}", entry.instance_name, e);
                continue;
            }
        };

        // Create the bi-directional channel before starting the server.
        let (hub_side, server_side) = messaging::create(&entry.instance_name);
        log_debug!("Channel created for server '{}'.", entry.instance_name);

        if let Err(e) = server.start(ServerContext {
            channel: server_side,
            registry: registry.clone() as Arc<dyn orkester_common::plugin::Registry>,
            executor_registry: Arc::clone(&executor_registry),
        }) {
            log_error!("Server '{}' failed to start: {}", entry.instance_name, e);
            continue;
        }

        log_info!("Server '{}' started.", entry.instance_name);

        hub_sides.push(hub_side);
        running.push(RunningServer {
            instance_name: entry.instance_name.clone(),
            component_key,
            server,
        });
    }

    if running.len() == entries.len() {
        log_info!("All {} server(s) started successfully.", running.len());
    } else {
        log_warn!(
            "{}/{} server(s) started — {} failed (see errors above).",
            running.len(),
            entries.len(),
            entries.len() - running.len()
        );
    }

    (running, hub_sides)
}

// ── Cleanup ───────────────────────────────────────────────────────────────────

/// Stop all running servers.
pub fn cleanup(servers: &[RunningServer]) -> Result<(), String> {
    if servers.is_empty() {
        log_info!("No running servers to stop.");
        return Ok(());
    }

    log_info!("Stopping {} server(s)...", servers.len());

    let mut errors: Vec<String> = Vec::new();

    for srv in servers {
        log_info!(
            "Stopping server '{}' ({})...",
            srv.instance_name,
            srv.component_key
        );
        if let Err(e) = srv.server.stop() {
            let msg = format!("Server '{}' stop error: {}", srv.instance_name, e);
            log_error!("{}", msg);
            errors.push(msg);
        } else {
            log_info!("Server '{}' stopped.", srv.instance_name);
        }
    }

    if errors.is_empty() {
        log_info!("All servers stopped.");
        Ok(())
    } else {
        Err(errors.join("; "))
    }
}
