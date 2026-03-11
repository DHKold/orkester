//! Server lifecycle management — parse config, build and start all enabled servers.

mod config;
mod runner;

pub use runner::RunningServer;

use crate::config::ConfigTree;
use crate::logging::Logger;
use crate::registry::Registry;

/// Parse server config, build and start all enabled servers.
pub fn start_servers(
    config: &ConfigTree,
    registry: &Registry,
) -> Result<Vec<RunningServer>, Box<dyn std::error::Error>> {
    let entries = config::parse(config);
    if entries.is_empty() {
        return Ok(Vec::new());
    }
    Ok(runner::start(&entries, registry))
}

/// Stop all running servers.
pub fn cleanup_servers(servers: &[RunningServer]) -> Result<(), String> {
    runner::cleanup(servers)
}
