use serde::Deserialize;
use serde_json::Value;

// ── Plugin directory config ───────────────────────────────────────────────────

#[derive(Debug, Deserialize, Clone)]
pub struct PluginDir {
    pub path: String,
}

#[derive(Debug, Deserialize, Default, Clone)]
pub struct PluginsConfig {
    #[serde(default)]
    pub directories: Vec<PluginDir>,
}

// ── Server (component instance) config ───────────────────────────────────────

#[derive(Debug, Deserialize, Default, Clone)]
pub struct StartActions {
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub config: Value,
}


#[derive(Debug, Deserialize, Clone)]
pub struct ServerConfig {
    pub name:   String,
    pub kind:   String,
    #[serde(default)]
    pub config: Value,
    #[serde(default)]
    pub start: Vec<StartActions>,
}

// ── Top-level host config ─────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Default)]
pub struct HostConfig {
    #[serde(default)]
    pub plugins: PluginsConfig,
    #[serde(default)]
    pub servers: Vec<ServerConfig>,
    #[serde(default)]
    pub hub:     orkester_plugin::hub::config::HubConfig,
}

impl HostConfig {
    pub fn from_yaml(s: &str) -> anyhow::Result<Self> {
        let cfg = serde_yaml::from_str(s)?;
        Ok(cfg)
    }
}
