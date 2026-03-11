//! Messaging types shared between the hub (core) and server implementations.
//!
//! Both [`Message`] and [`ServerSide`] are part of the plugin contract: every
//! [`Server`](crate::plugin::servers::Server) receives its [`ServerSide`]
//! channel when `start()` is called.

use std::fmt;
use std::sync::mpsc;

use serde_json::Value;

// ── Message ───────────────────────────────────────────────────────────────────

/// Every message flowing through Orkester's hub carries these fields.
#[derive(Debug, Clone)]
pub struct Message {
    /// Unique message ID (caller-assigned; the hub preserves it on forwarding).
    pub id: String,
    /// Name of the sender (server instance name or `"hub"`).
    pub source: String,
    /// Name of the intended recipient (server instance name or `"hub"`).
    pub target: String,
    /// Discriminator understood by the receiver (e.g. `"execute"`, `"shutdown"`).
    pub message_type: String,
    /// Arbitrary payload.
    pub content: Value,
}

impl Message {
    pub fn new(
        id: impl Into<String>,
        source: impl Into<String>,
        target: impl Into<String>,
        message_type: impl Into<String>,
        content: Value,
    ) -> Self {
        Message {
            id: id.into(),
            source: source.into(),
            target: target.into(),
            message_type: message_type.into(),
            content,
        }
    }

    /// Build an error reply sent back to the original sender when the target is unknown.
    pub fn unknown_target_error(original: &Message) -> Message {
        Message {
            id: format!("{}-err", original.id),
            source: "hub".to_string(),
            target: original.source.clone(),
            message_type: "error".to_string(),
            content: serde_json::json!({
                "error": "unknown_target",
                "original_target": original.target,
                "original_id": original.id,
            }),
        }
    }
}

impl fmt::Display for Message {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[{}] {} → {} ({})",
            self.id, self.source, self.target, self.message_type
        )
    }
}

// ── ServerSide ────────────────────────────────────────────────────────────────

/// The server's end of a bi-directional channel with the hub.
///
/// Passed to [`Server::start()`](crate::plugin::servers::Server::start) so the
/// server implementation can send and receive messages through the hub.
pub struct ServerSide {
    pub to_hub: mpsc::Sender<Message>,
    pub from_hub: mpsc::Receiver<Message>,
}
