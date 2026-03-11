//! Plugin loader — discovers and loads Orkester plugins from shared libraries.

mod loaded;

pub use loaded::LoadedPlugin;

use crate::config::ConfigTree;
use crate::logging::Logger;
use orkester_common::plugin::{Plugin, PluginRegistrationFn, PLUGIN_REGISTRATION_SYMBOL};
use std::path::{Path, PathBuf};

/// Scan `plugins.dir` for `.so` files and load each one as a plugin.
///
/// Config keys:
/// - `plugins.dir`       — directory to scan (required; skips loading if absent)
/// - `plugins.recursive` — descend into sub-directories (default: `false`)
///
/// Individual library failures are logged and skipped; they do **not** abort
/// the startup sequence.
pub fn load_plugins(config: &ConfigTree) -> Vec<LoadedPlugin> {
    let dir = match config.get_typed::<String>("plugins.dir") {
        Some(d) => d,
        None => {
            Logger::warn("No plugin directory configured under `plugins.dir` — running with no plugins.");
            return Vec::new();
        }
    };
    let recursive = config
        .get_typed::<bool>("plugins.recursive")
        .unwrap_or(false);
    Logger::info(&format!("Scanning plugin directory '{}' (recursive={})...", dir, recursive));

    let so_files = find_so_files(Path::new(&dir), recursive);
    if so_files.is_empty() {
        Logger::warn(&format!("No .so files found in plugin directory '{}'.", dir));
        return Vec::new();
    }
    Logger::info(&format!("Found {} .so file(s) — loading...", so_files.len()));

    let mut loaded: Vec<LoadedPlugin> = Vec::with_capacity(so_files.len());
    for path in &so_files {
        let display = path.display().to_string();
        Logger::trace(&format!("Attempting to load plugin library: {}", display));

        match load_dynamic(&display) {
            Ok(lp) => {
                let meta = &lp.plugin.metadata;
                Logger::info(&format!("Plugin loaded: '{}' v{} ({})", meta.id, meta.version, meta.description));
                loaded.push(lp);
            }
            Err(e) => {
                Logger::error(&format!("Failed to load plugin '{}': {}", display, e));
            }
        }
    }
    Logger::info(&format!("Plugin loading complete: {}/{} plugin(s) loaded successfully.", loaded.len(), so_files.len()));

    loaded
}

/// Collect all `.so` files in `dir`. If `recursive` is true, descends into
/// sub-directories.  Entries that cannot be read are logged and skipped.
fn find_so_files(dir: &Path, recursive: bool) -> Vec<PathBuf> {
    let mut results = Vec::new();

    let read_dir = match std::fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(e) => {
            Logger::error(&format!(
                "Cannot read plugin directory '{}': {}",
                dir.display(),
                e
            ));
            return results;
        }
    };

    for entry in read_dir {
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                Logger::warn(&format!(
                    "Skipping unreadable entry in '{}': {}",
                    dir.display(),
                    e
                ));
                continue;
            }
        };

        let path = entry.path();

        if path.is_dir() {
            if recursive {
                Logger::trace(&format!("Descending into sub-directory: {}", path.display()));
                results.extend(find_so_files(&path, recursive));
            }
        } else if path.extension().and_then(|e| e.to_str()) == Some("so") {
            Logger::trace(&format!("Found plugin file: {}", path.display()));
            results.push(path);
        }
    }

    results
}

/// Load a single plugin from a dynamic library at `path`.
///
/// # Safety
/// The library must export `orkester_register_plugin` with the expected ABI and
/// return a heap-allocated `Plugin` whose ownership is transferred to the
/// caller (i.e. created with `Box::into_raw`).
fn load_dynamic(path: &str) -> Result<LoadedPlugin, Box<dyn std::error::Error>> {
    // SAFETY: Loading untrusted shared libraries is inherently unsafe.
    // The library must match the expected ABI and export the correct symbol.
    let lib = unsafe { libloading::Library::new(path)? };

    let plugin: Box<Plugin> = unsafe {
        let sym: libloading::Symbol<PluginRegistrationFn> =
            lib.get(PLUGIN_REGISTRATION_SYMBOL.as_bytes())?;

        Logger::trace(&format!(
            "Symbol '{}' resolved in '{}'",
            PLUGIN_REGISTRATION_SYMBOL, path
        ));

        let raw: *mut Plugin = sym();
        if raw.is_null() {
            return Err(format!(
                "Plugin '{}' returned null from '{}'",
                path, PLUGIN_REGISTRATION_SYMBOL
            )
            .into());
        }

        Box::from_raw(raw)
    };

    Ok(LoadedPlugin {
        plugin,
        _lib: Some(lib),
    })
}
