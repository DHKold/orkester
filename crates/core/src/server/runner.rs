//! Build, start, and stop server instances.

use crate::messaging::{self, HubSide};
use crate::registry::Registry;
use orkester_common::logging;
use orkester_common::plugin::servers::Server;

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
pub fn start(entries: &[ServerEntry], registry: &Registry) -> (Vec<RunningServer>, Vec<HubSide>) {
    let mut running = Vec::with_capacity(entries.len());
    let mut hub_sides = Vec::with_capacity(entries.len());

    for entry in entries {
        let component_key = format!("{}:{}", entry.plugin_id, entry.server_id);

        logging::Logger::info(format!(
            "Starting server '{}' (component='{}')...",
            entry.instance_name, component_key
        ));

        let comp = match registry.server_builders.get(&component_key) {
            Some(c) => c,
            None => {
                logging::Logger::error(format!(
                    "No builder registered for '{}' (instance '{}'). \
                     Is the plugin providing it loaded?",
                    component_key, entry.instance_name
                ));
                continue;
            }
        };

        let builder = match &comp.builder {
            orkester_common::plugin::PluginComponent::Server(b) => b,
            _ => {
                logging::Logger::error(format!(
                    "Component '{}' is not a Server builder — skipping.",
                    component_key
                ));
                continue;
            }
        };

        logging::Logger::debug(format!(
            "Building server '{}' with config: {}",
            entry.instance_name, entry.config
        ));

        let server = match builder.build(entry.config.clone()) {
            Ok(s) => s,
            Err(e) => {
                logging::Logger::error(format!(
                    "Builder failed for server '{}': {}",
                    entry.instance_name, e
                ));
                continue;
            }
        };

        // Create the bi-directional channel before starting the server.
        let (hub_side, server_side) = messaging::create(&entry.instance_name);
        logging::Logger::debug(format!(
            "Channel created for server '{}'.",
            entry.instance_name
        ));

        if let Err(e) = server.start(server_side) {
            logging::Logger::error(format!(
                "Server '{}' failed to start: {}",
                entry.instance_name, e
            ));
            continue;
        }

        logging::Logger::info(format!("Server '{}' started.", entry.instance_name));

        hub_sides.push(hub_side);
        running.push(RunningServer {
            instance_name: entry.instance_name.clone(),
            component_key,
            server,
        });
    }

    if running.len() == entries.len() {
        logging::Logger::info(format!(
            "All {} server(s) started successfully.",
            running.len()
        ));
    } else {
        logging::Logger::warn(format!(
            "{}/{} server(s) started — {} failed (see errors above).",
            running.len(),
            entries.len(),
            entries.len() - running.len()
        ));
    }

    (running, hub_sides)
}

// ── Cleanup ───────────────────────────────────────────────────────────────────

/// Stop all running servers.
pub fn cleanup(servers: &[RunningServer]) -> Result<(), String> {
    if servers.is_empty() {
        logging::Logger::info("No running servers to stop.");
        return Ok(());
    }

    logging::Logger::info(format!("Stopping {} server(s)...", servers.len()));

    let mut errors: Vec<String> = Vec::new();

    for srv in servers {
        logging::Logger::info(format!(
            "Stopping server '{}' ({})...",
            srv.instance_name, srv.component_key
        ));
        if let Err(e) = srv.server.stop() {
            let msg = format!("Server '{}' stop error: {}", srv.instance_name, e);
            logging::Logger::error(&msg);
            errors.push(msg);
        } else {
            logging::Logger::info(format!("Server '{}' stopped.", srv.instance_name));
        }
    }

    if errors.is_empty() {
        logging::Logger::info("All servers stopped.");
        Ok(())
    } else {
        Err(errors.join("; "))
    }
}
