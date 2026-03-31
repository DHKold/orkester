use std::path::Path;
use std::collections::HashMap;
use serde_json::Value;

use anyhow::Result;
use walkdir::WalkDir;

use orkester_plugin::{log_error, log_warn, log_info, log_debug};
use orkester_plugin::abi::AbiComponent;
use orkester_plugin::sdk::{ComponentMetadata, Host, LoadedPlugin};

use crate::config::PluginsConfig;

// ── CatalogEntries ──────────────────────────────────────────────────────────────

pub struct CatalogPlugin {
    pub metadata: ComponentMetadata,
    pub plugin: LoadedPlugin,
}

pub struct CatalogComponent {
    pub metadata: ComponentMetadata,
    pub plugin_ref: String,
}

// ── Catalog ───────────────────────────────────────────────────────────────────

/// Scans configured directories and loads every dynamic library found.
pub struct Catalog {
    pub plugins: HashMap<String, CatalogPlugin>,
    pub components: HashMap<String, CatalogComponent>,
}

impl Catalog {
    /// Scan all plugin directories and return a [`Catalog`] with every
    /// successfully loaded plugin.  Libraries that fail to load are logged
    /// and skipped.
    pub fn load(host: &mut Host, cfg: &PluginsConfig) -> Result<Self> {
        let mut plugins = HashMap::new();
        let mut components = HashMap::new();
        log_info!("[catalog] Loading plugins from {} directories...", cfg.directories.len());

        for dir_cfg in &cfg.directories {
            let dir = Path::new(&dir_cfg.path);
            if !dir.exists() {
                log_warn!("[catalog] Plugin directory not found: {}", dir.display());
                continue;
            }

            for entry in WalkDir::new(dir).max_depth(1).follow_links(false) {
                let entry = match entry {
                    Ok(e) => e,
                    Err(e) => { log_warn!("[catalog] Failed to read directory entry: {e}"); continue; }
                };
                let path = entry.path();
                if !is_shared_lib(path) { continue; }

                log_debug!("[catalog] Loading plugin: {}", path.display());
                match host.load_plugin(path) {
                    Ok(mut plugin) => {
                        let meta = get_plugin_metadata(&mut plugin);
                        let plugin_ref = meta.kind.clone();
                        log_debug!("[catalog] Plugin metadata: kind='{}', name='{}', description='{}'", meta.kind, meta.name, meta.description);
                        
                        let plugin_components = get_plugin_childs(&mut plugin);
                        for comp_meta in &plugin_components {
                            let comp_ref = comp_meta.kind.clone();
                            log_debug!("[catalog] └─ Component: kind='{}', name='{}', description='{}'", comp_ref, comp_meta.name, comp_meta.description);
                            components.insert(comp_ref.clone(), CatalogComponent { metadata: comp_meta.clone(), plugin_ref: plugin_ref.clone() });
                        }
                        
                        plugins.insert(plugin_ref.clone(), CatalogPlugin { metadata: meta, plugin });
                    }
                    Err(e) => {
                        log_warn!("[catalog] Failed to load {}: {e}", path.display());
                    }
                }
            }
        }

        Ok(Self { plugins, components })
    }

    /// Instanciate a component of the given kind by calling the factory handler of the plugin that provides it.
    pub fn instantiate_component(&mut self, kind: &str, config: &Value) -> Result<*mut AbiComponent> {
        let req = serde_json::json!({
            "action": "orkester/CreateComponent",
            "params": { "kind": kind, "config": config }
        });

        // Look for the component kind in the catalog, then call its factory to get a live instance.
        let comp_entry = match self.components.get(kind) {
            Some(entry) => entry,
            None => {
                log_error!("[catalog] Component kind '{kind}' not found in catalog");
                return Err(anyhow::anyhow!("Component kind '{kind}' not found in catalog"));
            }
        };
        let plugin: &mut CatalogPlugin = match self.plugins.get_mut(&comp_entry.plugin_ref) {
            Some(plugin) => plugin,
            None => {
                log_error!("[catalog] Plugin '{}' not found for component kind '{kind}'", comp_entry.plugin_ref);
                return Err(anyhow::anyhow!("Plugin '{}' not found for component kind '{kind}'", comp_entry.plugin_ref));
            }
        };
        let plugin = &mut plugin.plugin;
        let mut handle = plugin.get_root_component();
        let component = match handle.call_factory(&req) {
            Ok(comp_ptr) => {
                log_info!("[catalog] Created component '{}' from plugin '{}'", kind, comp_entry.plugin_ref);
                comp_ptr
            }
            Err(e) => {
                log_error!("[catalog] Failed to create component of kind '{}' from plugin '{}': {}", kind, comp_entry.plugin_ref, e);
                return Err(anyhow::anyhow!("Failed to create component of kind '{kind}' from plugin '{}': {e}", comp_entry.plugin_ref));
            }
        };
        Ok(component)
    }
}

/// Extract the metadata of a plugin's root component.
fn get_plugin_metadata(plugin: &mut LoadedPlugin) -> ComponentMetadata {
    use orkester_plugin::sdk::message::Request;
    use serde_json::Value;
    let mut handle = plugin.get_root_component();
    let request = Request::new("orkester/GetMetadata", Value::Null);
    handle.call::<Request, ComponentMetadata>(&request).unwrap()
}

// Extract the metadata of a plugin's child components by calling its ListComponents handler.
fn get_plugin_childs(plugin: &mut LoadedPlugin) -> Vec<ComponentMetadata> {
    use orkester_plugin::sdk::message::Request;
    use serde_json::Value;
    let mut handle = plugin.get_root_component();
    let request = Request::new("orkester/ListComponents", Value::Null);
    handle.call::<Request, Vec<ComponentMetadata>>(&request).unwrap()
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn is_shared_lib(path: &Path) -> bool {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    matches!(ext, "so" | "dylib" | "dll")
}
