use std::path::{Path, PathBuf};
use libloading::{Library, Symbol};
use orkester_common::plugin::{Plugin, PLUGIN_REGISTRATION_SYMBOL};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PluginLoadError {
    #[error("Failed to load library '{path}': {source}")]
    Library {
        path: PathBuf,
        #[source]
        source: libloading::Error,
    },
    #[error("Symbol '{}' not found in '{path}': {source}", PLUGIN_REGISTRATION_SYMBOL)]
    Symbol {
        path: PathBuf,
        #[source]
        source: libloading::Error,
    },
    #[error("Plugin registration function returned a null pointer in '{0}'")]
    NullPointer(PathBuf),
}

/// A loaded plugin together with the library handle that must remain alive.
pub struct LoadedPlugin {
    pub plugin: Box<Plugin>,
    /// Keep the library alive for as long as the plugin is in use.
    _library: Library,
}

/// Scan `dir` for shared-library files and attempt to load each as an Orkester plugin.
///
/// Returns the list of successfully loaded plugins together with any errors encountered.
/// A single bad library does not abort the scan.
pub fn load_plugins(dir: &Path, recursive: bool) -> (Vec<LoadedPlugin>, Vec<PluginLoadError>) {
    let mut loaded = Vec::new();
    let mut errors = Vec::new();

    let candidates = collect_candidates(dir, recursive);

    for path in candidates {
        tracing::info!(path = %path.display(), "Loading plugin library");
        match load_one(&path) {
            Ok(plugin) => {
                tracing::info!(
                    path      = %path.display(),
                    plugin_id = %plugin.plugin.metadata.id,
                    version   = %plugin.plugin.metadata.version,
                    components = plugin.plugin.components.len(),
                    "Plugin loaded successfully"
                );
                loaded.push(plugin);
            }
            Err(e) => {
                tracing::warn!(path = %path.display(), error = %e, "Failed to load plugin, skipping");
                errors.push(e);
            }
        }
    }

    (loaded, errors)
}

// ── internals ──────────────────────────────────────────────────────────────

/// Collect all `.so` (Linux/macOS) and `.dll` (Windows) files from `dir`.
fn collect_candidates(dir: &Path, recursive: bool) -> Vec<PathBuf> {
    let mut out = Vec::new();
    collect_recursive(dir, recursive, &mut out);
    out
}

fn collect_recursive(dir: &Path, recursive: bool, out: &mut Vec<PathBuf>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(err) => {
            tracing::warn!(dir = %dir.display(), error = %err, "Cannot read plugins directory");
            return;
        }
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if recursive {
                collect_recursive(&path, recursive, out);
            }
        } else if is_shared_library(&path) {
            out.push(path);
        }
    }
}

fn is_shared_library(path: &Path) -> bool {
    match path.extension().and_then(|e| e.to_str()) {
        Some("so") | Some("dll") | Some("dylib") => true,
        _ => false,
    }
}

/// Load a single shared library and call its registration entry point.
///
/// # Safety
/// We trust the library to export a valid `orkester_register_plugin` symbol.
fn load_one(path: &Path) -> Result<LoadedPlugin, PluginLoadError> {
    // SAFETY: loading arbitrary native code.
    let library = unsafe {
        Library::new(path).map_err(|source| PluginLoadError::Library {
            path: path.to_path_buf(),
            source,
        })?
    };

    // SAFETY: we cast the raw pointer returned by the plugin to Box<Plugin>.
    let plugin = unsafe {
        let sym: Symbol<unsafe extern "C" fn() -> *mut Plugin> = library
            .get(PLUGIN_REGISTRATION_SYMBOL.as_bytes())
            .map_err(|source| PluginLoadError::Symbol {
                path: path.to_path_buf(),
                source,
            })?;

        let raw = sym();
        if raw.is_null() {
            return Err(PluginLoadError::NullPointer(path.to_path_buf()));
        }
        Box::from_raw(raw)
    };

    Ok(LoadedPlugin { plugin, _library: library })
}
