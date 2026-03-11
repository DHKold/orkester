//! Bi-directional channel between the hub and a single server instance.

use std::sync::mpsc;
pub use orkester_common::messaging::{Message, ServerSide};

/// The hub's end of a server channel.
///
/// The hub reads messages from `from_server` and writes to `to_server`.
pub struct HubSide {
    pub instance_name: String,
    pub to_server: mpsc::Sender<Message>,
    pub from_server: mpsc::Receiver<Message>,
}

/// Create a paired bi-directional channel for `instance_name`.
pub fn create(instance_name: impl Into<String>) -> (HubSide, ServerSide) {
    let (hub_to_srv_tx, hub_to_srv_rx) = mpsc::channel::<Message>();
    let (srv_to_hub_tx, srv_to_hub_rx) = mpsc::channel::<Message>();

    let hub_side = HubSide {
        instance_name: instance_name.into(),
        to_server: hub_to_srv_tx,
        from_server: srv_to_hub_rx,
    };

    let server_side = ServerSide {
        to_hub: srv_to_hub_tx,
        from_hub: hub_to_srv_rx,
    };

    (hub_side, server_side)
}
