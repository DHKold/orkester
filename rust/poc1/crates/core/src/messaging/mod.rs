//! Messaging system — bi-directional channels between the hub and each server.

mod channel;
mod hub;

pub use channel::{create, HubSide};
pub use hub::Hub;
