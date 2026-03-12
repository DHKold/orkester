//! Main config loader and extraction logic

use serde_json::Value;
use std::path::Path;

use super::access::ConfigTree;
use super::interface::ConfigLoader;
use super::json_loader::JsonConfigLoader;
use super::toml_loader::TomlConfigLoader;
use super::yaml_loader::YamlConfigLoader;

use orkester_common::logging;

/// Load and merge a list of config files, applying CLI overrides last.
/// Returns a merged [`ConfigTree`].
pub fn load_config_files(paths: &[&str], overrides: &[&str]) -> ConfigTree {
    logging::Logger::info(format!(
        "Loading config: {} file(s), {} override(s).",
        paths.len(),
        overrides.len()
    ));

    let mut merged = Value::Object(serde_json::Map::new());
    let mut loaded_count = 0usize;

    for path in paths {
        logging::Logger::debug(format!("Reading config file: {}", path));

        let ext = Path::new(path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        let loaded = match ext.as_str() {
            "json" => JsonConfigLoader::load(path),
            "yaml" | "yml" => YamlConfigLoader::load(path),
            "toml" => TomlConfigLoader::load(path),
            _ => {
                logging::Logger::warn(format!("Unknown config extension: {} (skipping)", ext));
                continue;
            }
        };
        match loaded {
            Ok(val) => {
                logging::Logger::debug(format!("Config file '{}' loaded successfully.", path));
                merge_values(&mut merged, &val);
                loaded_count += 1;
            }
            Err(e) => logging::Logger::error(format!("Failed to load config '{}': {}", path, e)),
        }
    }

    // Apply CLI overrides
    for ov in overrides {
        if let Some((k, v)) = ov.split_once('=') {
            logging::Logger::trace(format!("Applying config override: {} = {}", k, v));
            apply_override(&mut merged, k, v);
        } else {
            logging::Logger::warn(format!("Invalid override '{}', expected key=value", ov));
        }
    }

    logging::Logger::info(format!(
        "Config loading complete: {}/{} file(s) merged, {} override(s) applied.",
        loaded_count,
        paths.len(),
        overrides.len()
    ));
    logging::Logger::debug(format!(
        "Final config:\n{}",
        serde_json::to_string_pretty(&merged)
            .unwrap_or_else(|_| "<serialization error>".to_string())
    ));

    ConfigTree(merged)
}

/// Merge `src` into `dst`, overriding existing values (deep merge for objects)
fn merge_values(dst: &mut Value, src: &Value) {
    match (dst, src) {
        (Value::Object(dst_map), Value::Object(src_map)) => {
            for (k, v) in src_map {
                merge_values(dst_map.entry(k).or_insert(Value::Null), v);
            }
        }
        (dst_slot, src_val) => {
            *dst_slot = src_val.clone();
        }
    }
}

/// Apply a CLI override (dot-separated key) to the config tree
fn apply_override(root: &mut Value, key: &str, value: &str) {
    let mut parts = key.split('.').peekable();
    let mut current = root;
    while let Some(part) = parts.next() {
        if parts.peek().is_none() {
            // Last part, set value
            *current = match serde_json::from_str::<Value>(value) {
                Ok(v) => v,
                Err(_) => Value::String(value.to_string()),
            };
            return;
        }
        // Traverse or create object
        match current {
            Value::Object(map) => {
                current = map
                    .entry(part)
                    .or_insert(Value::Object(serde_json::Map::new()));
            }
            _ => {
                *current = Value::Object(serde_json::Map::new());
                if let Value::Object(map) = current {
                    current = map
                        .entry(part)
                        .or_insert(Value::Object(serde_json::Map::new()));
                }
            }
        }
    }
}
