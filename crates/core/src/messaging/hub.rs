//! The message hub — collects inbound messages from all servers and routes them
//! to the correct target, or returns an error message if the target is unknown.

use std::collections::HashMap;
use std::sync::mpsc;
use orkester_common::messaging::Message;

use crate::logging::Logger;

use super::channel::HubSide;

// ── Hub ───────────────────────────────────────────────────────────────────────

/// Central routing hub.
///
/// Owns one [`HubSide`] per registered server.  On each call to [`Hub::poll`]
/// it drains all pending inbound messages and forwards each one to the target
/// server, or returns an `"error"` message to the sender when the target is
/// not known.
pub struct Hub {
    /// Channels indexed by the server's instance name.
    channels: HashMap<String, HubSide>,
}

impl Hub {
    pub fn new() -> Self {
        Hub {
            channels: HashMap::new(),
        }
    }

    /// Register one server channel with the hub.
    pub fn register(&mut self, hub_side: HubSide) {
        Logger::debug(format!(
            "Hub: registered channel for server '{}'.",
            hub_side.instance_name
        ));
        self.channels
            .insert(hub_side.instance_name.clone(), hub_side);
    }

    /// Drain all pending inbound messages from every registered server and
    /// route each one.  Returns the number of messages processed.
    ///
    /// This is designed to be called repeatedly from the main loop.
    pub fn poll(&self) -> usize {
        let mut count = 0;

        for hub_side in self.channels.values() {
            loop {
                match hub_side.from_server.try_recv() {
                    Ok(mut msg) => {
                        // Always stamp the real sender before forwarding.
                        msg.source = hub_side.instance_name.clone();

                        Logger::trace(format!("Hub received: {}", msg));

                        self.route(msg);
                        count += 1;
                    }
                    Err(mpsc::TryRecvError::Empty) => break,
                    Err(mpsc::TryRecvError::Disconnected) => {
                        Logger::warn(format!(
                            "Hub: channel from server '{}' disconnected.",
                            hub_side.instance_name
                        ));
                        break;
                    }
                }
            }
        }

        count
    }

    // ── Internal routing ──────────────────────────────────────────────────────

    fn route(&self, msg: Message) {
        match self.channels.get(&msg.target) {
            Some(target_side) => {
                Logger::trace(format!("Hub forwarding: {}", msg));
                if let Err(e) = target_side.to_server.send(msg) {
                    Logger::error(format!("Hub: failed to deliver message to '{}': {}", e.0.target, e));
                }
            }
            None => {
                Logger::warn(format!(
                    "Hub: unknown target '{}' for message '{}' from '{}' — returning error.",
                    msg.target, msg.id, msg.source
                ));
                let error_reply = Message::unknown_target_error(&msg);
                self.send_error_reply(error_reply, &msg.source);
            }
        }
    }

    fn send_error_reply(&self, reply: Message, source: &str) {
        match self.channels.get(source) {
            Some(src_side) => {
                Logger::trace(format!("Hub: sending error reply to '{}'.", source));
                if let Err(e) = src_side.to_server.send(reply) {
                    Logger::error(format!(
                        "Hub: failed to deliver error reply to '{}': {}",
                        source, e
                    ));
                }
            }
            None => {
                // The original sender is also gone — just drop the error reply.
                Logger::warn(format!(
                    "Hub: could not send error reply — source '{}' is not registered.",
                    source
                ));
            }
        }
    }
}
