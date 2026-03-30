use serde::Deserialize;

/// Configuration for the MetricsServer component.
#[derive(Debug, Clone, Deserialize)]
pub struct MetricsServerConfig {
    /// When true, every recorded data point is kept in a time-ordered history.
    #[serde(default)]
    pub keep_history: bool,
    /// How long to keep history data, in seconds. Default: 1 hour.
    #[serde(default = "default_retention_secs")]
    pub retention_secs: u64,
    /// Maximum number of data points stored per minute per key.
    /// Lower values reduce memory at the cost of historical resolution.
    /// Default: 60 (one point per second).
    #[serde(default = "default_max_granularity_per_minute")]
    pub max_granularity_per_minute: u64,
}

impl Default for MetricsServerConfig {
    fn default() -> Self {
        Self {
            keep_history:               false,
            retention_secs:             default_retention_secs(),
            max_granularity_per_minute: default_max_granularity_per_minute(),
        }
    }
}

fn default_retention_secs() -> u64             { 3_600 }
fn default_max_granularity_per_minute() -> u64 { 60 }
