//! Server lifecycle management — parse config, resolve startup order, build and run.

use crate::config::ConfigTree;
use crate::logging::Logger;
use crate::registry::Registry;

/// Represents a running server instance, allowing for later shutdown.
pub struct RunningServer;

/// Start all servers defined in the config, using the provided plugin registry to construct them.
pub fn start_servers(config: &ConfigTree, registry: &Registry) -> Result<Vec<RunningServer>, Box<dyn std::error::Error>> {
    Ok(Vec::new()) // TODO: implement server startup logic
}

/// Clean up all running servers on shutdown.
pub fn cleanup_servers(servers: &[RunningServer]) -> Result<(), String> {
    Ok(())
}
