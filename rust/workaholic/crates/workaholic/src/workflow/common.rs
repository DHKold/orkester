use serde::{Deserialize, Serialize};

/// Trigger information recorded on a WorkRun or WorkRunRequest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trigger {
    /// Trigger type: `"manual"`, `"cron"`, `"webhook"`, etc.
    #[serde(rename = "type")]
    pub trigger_type: String,
    /// ISO 8601 timestamp when the trigger fired.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<String>,
    /// Identity that triggered the run (e.g. `"user:alice"`, `"cron:example-cron"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub identity: Option<String>,
}

/// A single entry in a resource's state transition history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateEvent {
    /// Name of the state reached.
    pub state: String,
    /// ISO 8601 timestamp when the transition occurred.
    pub timestamp: String,
    /// Human-readable explanation for the transition.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}
