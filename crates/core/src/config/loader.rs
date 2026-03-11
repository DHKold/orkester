//! Main config loader and extraction logic

use serde_json::Value;
use std::error::Error;
use std::path::Path;

use super::json_loader::JsonConfigLoader;
use super::yaml_loader::YamlConfigLoader;
use super::toml_loader::TomlConfigLoader;

/// Load and merge a list of config files, applying CLI overrides last.
/// Returns a merged config tree.
pub fn load_config_files(paths: &[&str], overrides: &[&str]) -> Value {
    let mut merged = Value::Object(serde_json::Map::new());
    for path in paths {
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
                tracing::warn!("Unknown config extension: {} (skipping)", ext);
                continue;
            }
        };
        match loaded {
            Ok(val) => merge_values(&mut merged, &val),
            Err(e) => tracing::error!("Failed to load config '{}': {}", path, e),
        }
    }
    // Apply CLI overrides
    for ov in overrides {
        if let Some((k, v)) = ov.split_once('=') {
            apply_override(&mut merged, k, v);
        } else {
            tracing::warn!("Invalid override '{}', expected key=value", ov);
        }
    }
    merged
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
                current = map.entry(part).or_insert(Value::Object(serde_json::Map::new()));
            }
            _ => {
                *current = Value::Object(serde_json::Map::new());
                if let Value::Object(map) = current {
                    current = map.entry(part).or_insert(Value::Object(serde_json::Map::new()));
                }
            }
        }
    }
}

/// Extract logging config from the config tree, if present.
pub fn extract_logging_config(config: &Value) -> Option<crate::logging::LoggingConfig> {
    // Example: expects { "logging": { "log_level": "debug", "log_format": "plain" } }
    let logging = config.get("logging")?;
    let log_level = logging.get("log_level").and_then(|v| v.as_str()).unwrap_or("info").to_string();
    let log_format = match logging.get("log_format").and_then(|v| v.as_str()) {
        Some("plain") => crate::logging::LogFormat::Plain,
        _ => crate::logging::LogFormat::Json,
    };
    Some(crate::logging::LoggingConfig { log_level, log_format })
}
