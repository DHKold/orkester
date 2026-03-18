use orkester_plugin::sdk::{Component, Host, Message, OwnedMessage, Result};
use orkester_plugin::sdk::protocol::ComponentMetadata;

pub struct UpperComponent;

impl UpperComponent {
    pub fn new() -> Self {
        Self
    }

    pub fn metadata() -> ComponentMetadata {
        ComponentMetadata {
            id: "upper".to_string(),
            name: Some("Uppercase Converter".to_string()),
            description: Some("Converts the input string to uppercase.".to_string()),
            input_types: vec![orkester_plugin::sdk::protocol::constants::MSG_TYPE_STRING],
            output_types: vec![orkester_plugin::sdk::protocol::constants::MSG_TYPE_STRING],
            extra: serde_json::Map::new(),
        }
    }
}

impl Component for UpperComponent {
    fn handle(&mut self, _host: Host, request: Message<'_>) -> Result<OwnedMessage> {
        let text = request.utf8()?.to_uppercase();

        Ok(OwnedMessage::new(
            request.id(),
            orkester_plugin::sdk::protocol::constants::MSG_TYPE_STRING,
            0,
            text.into_bytes(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use orkester_plugin::sdk::{Host, Message};
    use core::ptr;

    #[test]
    fn upper_converts_to_uppercase() {
        let mut comp = UpperComponent::new();
        let host = Host::new(ptr::null());
        let msg = Message::new(1, orkester_plugin::sdk::protocol::constants::MSG_TYPE_STRING, 0, b"Hello");
        let res = comp.handle(host, msg).expect("handle failed");
        assert_eq!(res.utf8().unwrap(), "HELLO");
        assert_eq!(res.type_id(), orkester_plugin::sdk::protocol::constants::MSG_TYPE_STRING);
    }
}