use orkester_plugin::sdk::{Component, Host, Message, OwnedMessage, Result};

use crate::protocol::ComponentDescriptor;

pub struct UpperComponent;

impl UpperComponent {
    pub fn new() -> Self {
        Self
    }

    pub fn descriptor() -> ComponentDescriptor {
        ComponentDescriptor {
            id: "upper".to_string(),
            name: "Upper Component".to_string(),
            description: "A simple component that converts incoming UTF-8 messages to uppercase.".to_string(),
        }
    }
}

impl Component for UpperComponent {
    fn handle(&mut self, _host: Host, request: Message<'_>) -> Result<OwnedMessage> {
        let text = request.utf8()?.to_uppercase();

        Ok(OwnedMessage::new(
            request.id(),
            orkester_plugin::abi::TYPE_UTF8,
            orkester_plugin::abi::FLAG_RESPONSE,
            text.into_bytes(),
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
    fn upper_converts_to_uppercase() {
        let mut comp = UpperComponent::new();
        let host = Host::new(ptr::null());
        let msg = Message::new(1, TYPE_UTF8, 0, b"Hello");
        let res = comp.handle(host, msg).expect("handle failed");
        assert_eq!(res.utf8().unwrap(), "HELLO");
        assert_eq!(res.type_id(), TYPE_UTF8);
    }
}