use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::HashMap, path::Path};

// ── Schema ────────────────────────────────────────────────────────────────────

/// Top-level host configuration.
#[derive(Debug, Default, Deserialize, Serialize)]
pub struct Config {
    #[serde(default)]
    pub plugins: PluginsConfig,
    #[serde(default)]
    pub servers: Vec<ServerConfig>,
    /// Catch-all for any other user-defined keys.
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct PluginsConfig {
    #[serde(default)]
    pub directories: Vec<PluginDirectory>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PluginDirectory {
    pub path: String,
    #[serde(default)]
    pub recursive: bool,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ServerConfig {
    /// Component kind to instantiate (e.g. `"sample/Logger:1.0"`).
    pub kind: String,
    /// Optional display name for log messages.
    pub name: Option<String>,
    /// Arbitrary config forwarded to the component factory.
    #[serde(default)]
    pub config: Value,
}

// ── Loading ───────────────────────────────────────────────────────────────────

/// Load one configuration file.  Supports `.json`, `.yaml`/`.yml`, `.toml`.
fn load_file(path: &Path) -> Result<Value> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("reading config file {}", path.display()))?;
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    match ext {
        "json" => serde_json::from_str(&content).map_err(Into::into),
        "yaml" | "yml" => serde_yaml::from_str(&content).map_err(Into::into),
        "toml" => {
            let v: toml::Value = toml::from_str(&content)?;
            Ok(serde_json::to_value(v)?)
        }
        other => anyhow::bail!("unsupported config extension: .{other}"),
    }
}

/// Deep-merge `overlay` into `base`.  Object keys from `overlay` override
/// same-named keys in `base`; other types are replaced entirely.
fn merge(base: &mut Value, overlay: Value) {
    match (base, overlay) {
        (Value::Object(b), Value::Object(o)) => {
            for (k, v) in o {
                merge(b.entry(k).or_insert(Value::Null), v);
            }
        }
        (base, overlay) => *base = overlay,
    }
}

/// Load and merge all config files in order, then apply key=value overrides.
///
/// Returns the fully-resolved [`Config`].
pub fn load(files: &[impl AsRef<Path>], overrides: &[String]) -> Result<Config> {
    let mut merged = Value::Object(Default::default());

    for f in files {
        let overlay = load_file(f.as_ref())?;
        merge(&mut merged, overlay);
    }

    for kv in overrides {
        let (key, value) = kv
            .split_once('=')
            .with_context(|| format!("--set value must be KEY=VALUE, got: {kv}"))?;
        let v: Value = serde_json::from_str(value)
            .unwrap_or_else(|_| Value::String(value.to_owned()));
        set_nested(&mut merged, key, v);
    }

    serde_json::from_value(merged).map_err(Into::into)
}

/// Set a dot-separated key path in a JSON Value.
///
/// e.g. `plugins.directories.0.recursive` = `true`
fn set_nested(root: &mut Value, key: &str, value: Value) {
    let parts: Vec<&str> = key.splitn(2, '.').collect();
    if parts.len() == 1 {
        if let Value::Object(map) = root {
            map.insert(key.to_owned(), value);
        }
        return;
    }
    let head = parts[0];
    let tail = parts[1];
    if let Value::Object(map) = root {
        set_nested(map.entry(head).or_insert_with(|| Value::Object(Default::default())), tail, value);
    }
}
