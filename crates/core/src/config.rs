use std::path::PathBuf;
use serde_json::Value;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Could not read config file '{path}': {source}")]
    ReadFile {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("Unsupported config file format: '{0}' (supported: .json, .yaml, .yml, .toml)")]
    UnsupportedFormat(String),
    #[error("Failed to parse config file '{path}': {message}")]
    Parse { path: PathBuf, message: String },
}

#[derive(Debug, Clone, Default)]
pub struct AppConfig(Value);

impl AppConfig {
    /// Parse a config file into the generic tree.
    /// The format is inferred from the file extension.
    pub fn from_file(path: &PathBuf) -> Result<Self, ConfigError> {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        let raw = std::fs::read_to_string(path).map_err(|source| ConfigError::ReadFile {
            path: path.clone(),
            source,
        })?;

        // Parse each format directly into serde_json::Value so the rest of the
        // application sees one unified tree regardless of the source format.
        let value: Value = match ext.as_str() {
            "json" => serde_json::from_str(&raw).map_err(|e| ConfigError::Parse {
                path: path.clone(),
                message: e.to_string(),
            })?,
            "yaml" | "yml" => serde_yaml::from_str(&raw).map_err(|e| ConfigError::Parse {
                path: path.clone(),
                message: e.to_string(),
            })?,
            "toml" => {
                let toml_val: toml::Value =
                    toml::from_str(&raw).map_err(|e| ConfigError::Parse {
                        path: path.clone(),
                        message: e.to_string(),
                    })?;
                // Convert via JSON round-trip (both use serde)
                serde_json::to_value(toml_val).map_err(|e| ConfigError::Parse {
                    path: path.clone(),
                    message: e.to_string(),
                })?
            }
            other => return Err(ConfigError::UnsupportedFormat(format!(".{}", other))),
        };

        Ok(Self(value))
    }

    /// Look up a value by a dot-separated path (e.g. `"plugins.dir"`).
    /// Returns `None` if any segment is absent.
    pub fn get(&self, path: &str) -> Option<&Value> {
        get_path(&self.0, path)
    }

    /// Convenience: string value at `path`, or `default` if absent / not a string.
    pub fn get_str<'a>(&'a self, path: &str, default: &'static str) -> &'a str {
        self.get(path).and_then(|v| v.as_str()).unwrap_or(default)
    }

    /// Convenience: bool value at `path`, or `default` if absent / not a bool.
    pub fn get_bool(&self, path: &str, default: bool) -> bool {
        self.get(path).and_then(|v| v.as_bool()).unwrap_or(default)
    }

    /// Raw inner value (e.g. to pass a sub-tree to a plugin factory).
    pub fn value(&self) -> &Value {
        &self.0
    }
}

/// Walk a dot-separated path through a `Value` tree.
fn get_path<'a>(mut cur: &'a Value, path: &str) -> Option<&'a Value> {
    for segment in path.split('.') {
        cur = cur.get(segment)?;
    }
    Some(cur)
}
