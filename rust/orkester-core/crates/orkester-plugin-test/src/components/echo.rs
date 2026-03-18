use orkester_plugin::sdk::{Component, Host, Message, OwnedMessage, Result};
use orkester_plugin::sdk::protocol::ComponentMetadata;

pub struct EchoComponent;

impl EchoComponent {
    pub fn new() -> Self {
        Self
    }

    pub fn metadata() -> ComponentMetadata {
        ComponentMetadata {
            id: "echo".to_string(),
            name: Some("Echo".to_string()),
            description: Some("Returns the received payload unchanged".to_string()),
            input_types: vec![orkester_plugin::sdk::protocol::constants::MSG_TYPE_STRING],
            output_types: vec![orkester_plugin::sdk::protocol::constants::MSG_TYPE_STRING],
            extra: serde_json::Map::new(),
        }
    }
}

impl Component for EchoComponent {
    fn handle(&mut self, _host: Host, request: Message<'_>) -> Result<OwnedMessage> {
        Ok(OwnedMessage::new(
            request.id(),
            request.type_id(),
            0,
            request.payload().to_vec(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use orkester_plugin::sdk::{Host, Message};
    use crate::protocol::TYPE_UTF8;
    use core::ptr;

    #[test]
    fn echo_returns_same_payload() {
        let mut comp = EchoComponent::new();
        let host = Host::new(ptr::null());
        let payload = b"hello";
        let msg = Message::new(1, orkester_plugin::sdk::protocol::constants::MSG_TYPE_STRING, 0, payload);
        let res = comp.handle(host, msg).expect("handle failed");
        assert_eq!(res.payload(), payload);
        assert_eq!(res.type_id(), orkester_plugin::sdk::protocol::constants::MSG_TYPE_STRING);
    }
}