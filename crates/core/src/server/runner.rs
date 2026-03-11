//! Build, start, and stop server instances.

use crate::logging::Logger;
use crate::registry::Registry;
use orkester_common::plugin::servers::Server;

use super::config::ServerEntry;

// ── RunningServer ─────────────────────────────────────────────────────────────

/// A server that has been built and started.
///
/// **Drop order**: `server` before `_` — the `Box<dyn Server>` is dropped first,
/// which triggers `stop()` via `Drop` if the implementation uses it internally.
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
/// Entries for which no matching builder is found are logged as errors and
/// skipped — they do **not** abort startup of subsequent servers.
pub fn start(entries: &[ServerEntry], registry: &Registry) -> Vec<RunningServer> {
    let mut running = Vec::with_capacity(entries.len());

    for entry in entries {
        let component_key = format!("{}:{}", entry.plugin_id, entry.server_id);

        Logger::info(format!(
            "Starting server '{}' (component='{}')...",
            entry.instance_name, component_key
        ));

        let comp = match registry.server_builders.get(&component_key) {
            Some(c) => c,
            None => {
                Logger::error(format!(
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
                Logger::error(format!(
                    "Component '{}' is not a Server builder — skipping.",
                    component_key
                ));
                continue;
            }
        };

        Logger::debug(format!(
            "Building server '{}' with config: {}",
            entry.instance_name,
            entry.config
        ));

        let server = match builder.build(entry.config.clone()) {
            Ok(s) => s,
            Err(e) => {
                Logger::error(format!(
                    "Builder failed for server '{}': {}",
                    entry.instance_name, e
                ));
                continue;
            }
        };

        if let Err(e) = server.start() {
            Logger::error(format!(
                "Server '{}' failed to start: {}",
                entry.instance_name, e
            ));
            continue;
        }

        Logger::info(format!("Server '{}' started.", entry.instance_name));

        running.push(RunningServer {
            instance_name: entry.instance_name.clone(),
            component_key,
            server,
        });
    }

    if running.len() == entries.len() {
        Logger::info(format!(
            "All {} server(s) started successfully.",
            running.len()
        ));
    } else {
        Logger::warn(format!(
            "{}/{} server(s) started — {} failed (see errors above).",
            running.len(),
            entries.len(),
            entries.len() - running.len()
        ));
    }

    running
}

// ── Cleanup ───────────────────────────────────────────────────────────────────

/// Stop all running servers.
pub fn cleanup(servers: &[RunningServer]) -> Result<(), String> {
    if servers.is_empty() {
        Logger::info("No running servers to stop.");
        return Ok(());
    }

    Logger::info(format!("Stopping {} server(s)...", servers.len()));

    let mut errors: Vec<String> = Vec::new();

    for srv in servers {
        Logger::info(format!(
            "Stopping server '{}' ({})...",
            srv.instance_name, srv.component_key
        ));
        if let Err(e) = srv.server.stop() {
            let msg = format!("Server '{}' stop error: {}", srv.instance_name, e);
            Logger::error(&msg);
            errors.push(msg);
        } else {
            Logger::info(format!("Server '{}' stopped.", srv.instance_name));
        }
    }

    if errors.is_empty() {
        Logger::info("All servers stopped.");
        Ok(())
    } else {
        Err(errors.join("; "))
    }
}
