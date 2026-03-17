use orkester_plugin::sdk::{Component, Host, Message, OwnedMessage, Result};

pub struct UpperComponent;

impl UpperComponent {
    pub fn new() -> Self {
        Self
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