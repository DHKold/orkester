use anyhow::{Context, Result};
use orkester_plugin::{abi::AbiComponent, sdk::{Host, LoadedPlugin, message::Deserializer}};
use std::path::Path;
use walkdir::WalkDir;

use crate::config::{PluginDirectory, ServerConfig};

// ── Plugin entry ──────────────────────────────────────────────────────────────

/// A loaded plugin with its root component.
struct Plugin {
    _plugin: LoadedPlugin,
    root: *mut AbiComponent,
}

// SAFETY: we access `root` only from a single thread (the host main thread).
unsafe impl Send for Plugin {}

// ── Catalog ───────────────────────────────────────────────────────────────────

/// Holds all loaded plugins and provides component instantiation.
pub struct Catalog {
    plugins: Vec<Plugin>,
}

impl Catalog {
    /// Scan `directories` for shared libraries, load each as an Orkester plugin.
    pub fn load(host: &mut Host, directories: &[PluginDirectory]) -> Result<Self> {
        let mut plugins = Vec::new();

        for dir in directories {
            let walker = if dir.recursive {
                WalkDir::new(&dir.path)
            } else {
                WalkDir::new(&dir.path).max_depth(1)
            };

            for entry in walker {
                let entry = entry.with_context(|| format!("walking {}", dir.path))?;
                if !entry.file_type().is_file() {
                    continue;
                }
                let path = entry.path();
                if !is_plugin_lib(path) {
                    continue;
                }
                match host.load_plugin(path) {
                    Ok(loaded) => {
                        let root = loaded.root_ptr();
                        plugins.push(Plugin { _plugin: loaded, root });
                        eprintln!("[catalog] loaded plugin: {}", path.display());
                    }
                    Err(e) => {
                        eprintln!("[catalog] skipping {}: {e}", path.display());
                    }
                }
            }
        }

        Ok(Self { plugins })
    }

    /// Attempt to instantiate `kind` from any loaded plugin root component.
    ///
    /// Iterates all roots and sends an `"orkester/CreateComponent"` request;
    /// returns the first successfully created component, or an error.
    pub fn create_component(
        &mut self,
        _host: &mut Host,
        server: &ServerConfig,
    ) -> Result<*mut AbiComponent> {
        use orkester_plugin::sdk::message::{Serializer, envelope::CreateComponentRequest};

        let req_body = CreateComponentRequest::with_config(server.kind.clone(), &server.config);

        // Wrap in the standard action envelope.
        let envelope = serde_json::json!({
            "action": "orkester/CreateComponent",
            "params": serde_json::to_value(&req_body)?
        });
        let envelope_req = Serializer::json(&envelope);

        for plugin in &mut self.plugins {
            let raw_res = unsafe { ((*plugin.root).handle)(plugin.root, envelope_req.as_abi()) };
            match Deserializer::component(plugin.root, raw_res) {
                Ok(ptr) => return Ok(ptr),
                Err(_) => continue,
            }
        }
        anyhow::bail!("no plugin provides component kind '{}'", server.kind)
    }
}

fn is_plugin_lib(path: &Path) -> bool {
    path.extension().map_or(false, |ext| {
        ext == "so" || ext == "dll" || ext == "dylib"
    })
}
