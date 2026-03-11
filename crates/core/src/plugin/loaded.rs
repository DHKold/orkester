use libloading::Library;
use orkester_common::plugin::Plugin;

/// A fully loaded plugin together with (for dynamic libraries) its backing
/// shared-library handle.
///
/// **Drop order is significant**: `plugin` is declared *before* `_lib` so Rust
/// drops `plugin` first — releasing all vtable-backed trait objects — before
/// the library is unloaded.
pub struct LoadedPlugin {
    /// The plugin definition returned by its registration function.
    pub plugin: Box<Plugin>,
    /// Owning handle for the shared library (`None` for statically-linked plugins).
    pub(super) _lib: Option<Library>,
}
