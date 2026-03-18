//! Concrete [`HostHandler`] implementation for the Orkester host binary.
//!
//! Plugins communicate back to the host by calling its `handle` function
//! pointer.  This module provides the default implementation: it decodes
//! the event payload and prints it to stderr, giving operators visibility
//! into what plugins are doing.
//!
//! The response from the host to the plugin is always an empty
//! `MSG_TYPE_BYTES` payload, which plugins treat as "acknowledged".

use orkester_plugin::sdk::{
    FLAG_NONE, MSG_TYPE_BYTES, MSG_TYPE_JSON, MSG_TYPE_STRING, HostHandler,
};

/// Prints every plugin callback to stderr and returns an empty acknowledgement.
///
/// The expected payload format for plugin log events is:
/// ```json
/// { "type": "Log", "message": "<text>" }
/// ```
/// Any other format is printed as raw bytes or a plain string.
pub struct LoggingHostHandler;

impl HostHandler for LoggingHostHandler {
    fn handle(&self, id: u64, format: u32, _flags: u32, payload: &[u8]) -> (Vec<u8>, u32, u32) {
        match format {
            MSG_TYPE_JSON => {
                if let Ok(text) = std::str::from_utf8(payload) {
                    // Try to extract a "message" field (the standard Log event shape).
                    if let Ok(v) = serde_json::from_str::<serde_json::Value>(text) {
                        if let Some(msg) = v.get("message").and_then(|m| m.as_str()) {
                            eprintln!("[plugin:{id}] {msg}");
                        } else {
                            eprintln!("[plugin:{id}] {text}");
                        }
                    } else {
                        eprintln!("[plugin:{id}] {text}");
                    }
                }
            }
            MSG_TYPE_STRING => {
                if let Ok(text) = std::str::from_utf8(payload) {
                    eprintln!("[plugin:{id}] {text}");
                }
            }
            _ => {
                eprintln!("[plugin:{id}] <binary callback, {} bytes>", payload.len());
            }
        }
        (Vec::new(), MSG_TYPE_BYTES, FLAG_NONE)
    }
}
