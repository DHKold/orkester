use std::path::PathBuf;
use serde::Deserialize;
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

/// Configuration for plugin discovery.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct PluginsConfig {
    /// Directory to scan for plugin shared libraries.
    #[serde(default = "default_plugins_dir")]
    pub dir: PathBuf,
    /// Whether to scan the directory recursively.
    #[serde(default)]
    pub recursive: bool,
}

impl Default for PluginsConfig {
    fn default() -> Self {
        Self {
            dir: default_plugins_dir(),
            recursive: false,
        }
    }
}

fn default_plugins_dir() -> PathBuf {
    PathBuf::from("./plugins")
}

/// Top-level application configuration.
/// Can be supplied as a JSON, YAML or TOML file via the `-c / --config-file` CLI flag.
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub struct AppConfig {
    #[serde(default)]
    pub plugins: PluginsConfig,
}

impl AppConfig {
    /// Load configuration from `path`.
    /// The format is determined by the file extension.
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

        let config: AppConfig = match ext.as_str() {
            "json" => serde_json::from_str(&raw).map_err(|e| ConfigError::Parse {
                path: path.clone(),
                message: e.to_string(),
            })?,
            "yaml" | "yml" => serde_yaml::from_str(&raw).map_err(|e| ConfigError::Parse {
                path: path.clone(),
                message: e.to_string(),
            })?,
            "toml" => toml::from_str(&raw).map_err(|e| ConfigError::Parse {
                path: path.clone(),
                message: e.to_string(),
            })?,
            other => return Err(ConfigError::UnsupportedFormat(format!(".{}", other))),
        };

        Ok(config)
    }
}
