//! Server management

use crate::config::ConfigTree;
use crate::registry::Registry;

/// A running server handle (placeholder until server startup is implemented).
pub struct RunningServer;

/// Start all servers whose factories are registered in `registry`.
pub fn start_servers(
    _config: &ConfigTree,
    _registry: &Registry,
) -> Result<Vec<RunningServer>, Box<dyn std::error::Error>> {
    // TODO: Topological sort of server_factories by dependencies, build, and run each.
    Ok(vec![])
}

/// Shut down all running servers cleanly.
pub fn cleanup_servers(_servers: &[RunningServer]) -> Result<(), String> {
    // TODO: Implement real cleanup
    Ok(())
}
