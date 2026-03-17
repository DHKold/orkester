use orkester_plugin::sdk::{Component, Host, Message, OwnedMessage, Result};

use crate::protocol::ComponentDescriptor;

pub struct EchoComponent;

impl EchoComponent {
    pub fn new() -> Self {
        Self
    }

    pub fn descriptor() -> ComponentDescriptor {
        ComponentDescriptor {
            id: "echo".to_string(),
            name: "Echo Component".to_string(),
            description: "A simple component that echoes back the received message.".to_string(),
        }
    }
}

impl Component for EchoComponent {
    fn handle(&mut self, _host: Host, request: Message<'_>) -> Result<OwnedMessage> {
        Ok(OwnedMessage::new(
            request.id(),
            request.type_id(),
            orkester_plugin::abi::FLAG_RESPONSE,
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
        let msg = Message::new(1, TYPE_UTF8, 0, payload);
        let res = comp.handle(host, msg).expect("handle failed");
        assert_eq!(res.payload(), payload);
        assert_eq!(res.type_id(), TYPE_UTF8);
    }
}