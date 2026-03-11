//! Component and server registries

// TODO: Implement registries for components and servers
// TODO: Support dependency resolution
// TODO: Provide lookup and lifecycle management

use crate::plugin::LoadedPlugin;

/// Register the components declared by each loaded plugin.
pub fn register_plugins(_plugins: &[LoadedPlugin]) {
    // TODO: Implement real registration
}
