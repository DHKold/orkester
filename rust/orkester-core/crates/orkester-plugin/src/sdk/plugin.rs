use std::path::Path;

use libloading::Library;

use crate::abi;
use super::component::Component;
use super::error::Error;
use super::host::{HostHandler, NullHostHandler, OrkesterHost};

/// The symbol that every Orkester plugin shared library must export.
///
/// Its signature must match [`abi::FnComponentBuilder`]:
/// ```c
/// abi::Component* orkester_create_root(abi::Host* host);
/// ```
pub const COMPONENT_BUILDER_SYMBOL: &[u8] = b"orkester_create_root\0";

// ─── Plugin ───────────────────────────────────────────────────────────────────

/// A loaded plugin and its full lifecycle owner.
///
/// Manages the following in strict dependency order:
///
/// 1. `host`     — the [`OrkesterHost`] passed to the plugin on load (stays
///                 alive so the plugin can call back at any time).
/// 2. `root`     — the root [`Component`] returned by the plugin; freed first
///                 on drop (before the library is unloaded).
/// 3. `_library` — the [`Library`] handle that keeps the SO/DLL mapped; dropped
///                 last so function pointers in `root` remain valid throughout.
///
/// Because the host is backed by heap-stable `Box<abi::Host>` storage (see
/// [`OrkesterHost`]), moving a `Plugin` after construction is safe.
pub struct Plugin {
    root: Component,
    /// Kept alive so the host's heap-stable storage (and any registered
    /// callbacks) outlive the root component; must be declared *after* `root`
    /// so Rust drops `root` first.
    _host: OrkesterHost,
    /// Kept alive so all symbols referenced by `root` remain valid; must be
    /// declared *after* `root` so Rust drops `root` before unloading the lib.
    _library: Library,
}

impl Plugin {
    /// Load a plugin from `path`, call its `orkester_create_root` export, and
    /// return the initialised `Plugin`.
    ///
    /// The plugin will not be able to call back into the host (the no-op
    /// [`NullHostHandler`] is used).  Call [`Plugin::load_with_handler`] if you
    /// need host callbacks.
    pub fn load(path: impl AsRef<Path>) -> Result<Self, Error> {
        Self::load_with_symbol_and_handler(path, COMPONENT_BUILDER_SYMBOL, NullHostHandler)
    }

    /// Load a plugin with a custom [`HostHandler`] for plugin → host callbacks.
    pub fn load_with_handler(
        path: impl AsRef<Path>,
        handler: impl HostHandler,
    ) -> Result<Self, Error> {
        Self::load_with_symbol_and_handler(path, COMPONENT_BUILDER_SYMBOL, handler)
    }

    /// Load a plugin, resolving the root-component builder under `symbol`
    /// (null-terminated byte string) instead of the default
    /// [`COMPONENT_BUILDER_SYMBOL`].
    pub fn load_with_symbol_and_handler(
        path: impl AsRef<Path>,
        symbol: &[u8],
        handler: impl HostHandler,
    ) -> Result<Self, Error> {
        // SAFETY: loading arbitrary shared libraries is inherently unsafe; the
        // caller is responsible for providing a well-formed Orkester plugin.
        let library = unsafe { Library::new(path.as_ref())? };

        let mut host = OrkesterHost::new(handler);

        let root = {
            // SAFETY: `symbol` is a null-terminated byte slice naming a function
            // whose type matches `FnComponentBuilder`.
            let builder: libloading::Symbol<abi::FnComponentBuilder> =
                unsafe { library.get(symbol)? };
            // SAFETY: we call the builder exactly once; `host.as_ptr()` is a
            // stable pointer valid until `host` is dropped (which happens after
            // `library` in this struct's drop order).
            let raw = unsafe { (*builder)(host.as_ptr()) };
            // SAFETY: the plugin promises a valid, non-null component on success.
            unsafe { Component::from_raw(raw)? }
        };

        Ok(Self { root, _host: host, _library: library })
    }

    /// Returns a reference to the root component.
    pub fn root(&self) -> &Component {
        &self.root
    }

    /// Returns a mutable reference to the root component.
    pub fn root_mut(&mut self) -> &mut Component {
        &mut self.root
    }
}
