use orkester_plugin::sdk::{Component, Host, Message, OwnedMessage, Result};

pub struct EchoComponent;

impl EchoComponent {
    pub fn new() -> Self {
        Self
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