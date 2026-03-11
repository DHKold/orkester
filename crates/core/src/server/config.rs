//! Parse `servers.*` entries from the config tree.

use crate::config::ConfigTree;
use crate::logging::Logger;
use serde_json::Value;

/// A single server entry parsed from the `servers` config block.
pub struct ServerEntry {
    /// Config key used as the instance name (e.g. `"rest_api"`, `"metrics"`).
    pub instance_name: String,
    /// Plugin id — used to look up the builder in the registry (e.g. `"orkester-plugin-core"`).
    pub plugin_id: String,
    /// Server component id matching `ComponentMetadata::id` (e.g. `"axum-rest-server"`).
    pub server_id: String,
    /// Server-specific config subtree passed verbatim to `ServerBuilder::build()`.
    pub config: Value,
}

/// Read and parse all entries under `servers.*` from the config tree.
/// Entries with `enabled: false` are filtered out.
pub fn parse(config: &ConfigTree) -> Vec<ServerEntry> {
    let servers_value = match config.get("servers") {
        Some(v) => v,
        None => {
            Logger::info("No `servers` block in config — nothing to start.");
            return Vec::new();
        }
    };

    let servers_map = match servers_value.as_object() {
        Some(m) => m,
        None => {
            Logger::error("`servers` config entry must be an object map.");
            return Vec::new();
        }
    };

    let mut entries = Vec::new();

    for (instance_name, entry_value) in servers_map {
        let enabled = entry_value
            .get("enabled")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        if !enabled {
            Logger::debug(format!(
                "Server '{}' has enabled=false — skipping.",
                instance_name
            ));
            continue;
        }

        let component = entry_value.get("component");

        let plugin_id = component
            .and_then(|c| c.get("plugin"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let server_id = match component.and_then(|c| c.get("server")).and_then(|v| v.as_str()) {
            Some(n) => n.to_string(),
            None => {
                Logger::error(format!(
                    "Server entry '{}' is missing `component.server` — skipping.",
                    instance_name
                ));
                continue;
            }
        };

        // Pass everything except the `component` key as the server's own config.
        let server_config = {
            let mut cfg = entry_value.clone();
            if let Some(obj) = cfg.as_object_mut() {
                obj.remove("component");
                obj.remove("enabled");
            }
            cfg
        };

        Logger::debug(format!(
            "Parsed server entry '{}' → plugin='{}' server='{}'",
            instance_name, plugin_id, server_id
        ));

        entries.push(ServerEntry {
            instance_name: instance_name.clone(),
            plugin_id,
            server_id,
            config: server_config,
        });
    }

    Logger::info(format!(
        "Config contains {} enabled server(s).",
        entries.len()
    ));

    entries
}
