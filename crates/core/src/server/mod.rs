//! Server management

use crate::config::ConfigTree;

/// Stub: Start servers
pub fn start_servers(_config: &ConfigTree) -> Vec<()> {
    // TODO: Implement real server startup
    vec![]
}

/// Stub: Cleanup servers
pub fn cleanup_servers(_servers: &Vec<()>) -> Result<(), String> {
    // TODO: Implement real cleanup
    Ok(())
}
