use std::path::Path;

use anyhow::Result;
use walkdir::WalkDir;

use orkester_plugin::sdk::{Host, LoadedPlugin};

use crate::config::PluginsConfig;

// ── CatalogEntry ──────────────────────────────────────────────────────────────

/// A loaded plugin library with its root component alive and ready.
pub struct CatalogEntry {
    /// User-friendly name derived from the library file stem.
    pub name:   String,
    pub plugin: LoadedPlugin,
}

// ── Catalog ───────────────────────────────────────────────────────────────────

/// Scans configured directories and loads every dynamic library found.
pub struct Catalog {
    pub entries: Vec<CatalogEntry>,
}

impl Catalog {
    /// Scan all plugin directories and return a [`Catalog`] with every
    /// successfully loaded plugin.  Libraries that fail to load are logged
    /// and skipped.
    pub fn load(host: &mut Host, cfg: &PluginsConfig) -> Result<Self> {
        let mut entries = Vec::new();

        for dir_cfg in &cfg.directories {
            let dir = Path::new(&dir_cfg.path);
            if !dir.exists() {
                log::warn!("[catalog] plugin directory not found: {}", dir.display());
                continue;
            }

            for entry in WalkDir::new(dir).max_depth(1).follow_links(false) {
                let entry = match entry {
                    Ok(e) => e,
                    Err(e) => { log::warn!("[catalog] walk error: {e}"); continue; }
                };
                let path = entry.path();
                if !is_shared_lib(path) { continue; }

                log::debug!("[catalog] loading plugin: {}", path.display());
                match host.load_plugin(path) {
                    Ok(plugin) => {
                        let name = path.file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or("unknown")
                            .to_owned();
                        log::info!("[catalog] loaded plugin '{name}' from {}", path.display());
                        entries.push(CatalogEntry { name, plugin });
                    }
                    Err(e) => {
                        log::warn!("[catalog] failed to load {}: {e}", path.display());
                    }
                }
            }
        }

        Ok(Self { entries })
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn is_shared_lib(path: &Path) -> bool {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    matches!(ext, "so" | "dylib" | "dll")
}
