use serde::Deserialize;
use serde_json::Value;

// ── Backpressure ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum BackpressurePolicy {
    /// Return an error immediately (default).
    #[default]
    Reject,
    /// Block the caller until a slot is available.
    Block,
    /// Silently discard the incoming message.
    DropNewest,
    /// Discard the oldest queued message to make room.
    DropOldest,
}

// ── Queue config ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct QueueConfig {
    #[serde(default = "default_waiting")]
    pub waiting_capacity: usize,
    #[serde(default = "default_dispatch")]
    pub dispatch_capacity: usize,
    #[serde(default)]
    pub backpressure: BackpressurePolicy,
}

impl Default for QueueConfig {
    fn default() -> Self {
        Self {
            waiting_capacity: default_waiting(),
            dispatch_capacity: default_dispatch(),
            backpressure: BackpressurePolicy::Reject,
        }
    }
}

fn default_waiting()  -> usize { 4_096 }
fn default_dispatch() -> usize { 512 }

// ── Route config types ────────────────────────────────────────────────────────

/// Extensible filter.  Kind determines interpretation of `config`.
#[derive(Debug, Clone, Deserialize)]
pub struct FilterConfig {
    pub kind: String,
    #[serde(default)]
    pub config: Value,
}

/// Extensible target.  Kind determines interpretation of `config`.
#[derive(Debug, Clone, Deserialize)]
pub struct TargetConfig {
    pub kind: String,
    #[serde(default)]
    pub config: Value,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RouteRuleConfig {
    pub name: String,
    /// A message matches if ANY filter returns true.
    pub filters: Vec<FilterConfig>,
    /// On match, the message is sent to ALL targets.
    pub targets: Vec<TargetConfig>,
}

// ── Top-level hub config ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Default)]
pub struct HubConfig {
    #[serde(default)]
    pub queue: QueueConfig,
    /// Route rules listed directly as a sequence (no wrapping object needed).
    #[serde(default)]
    pub routes: Vec<RouteRuleConfig>,
}
