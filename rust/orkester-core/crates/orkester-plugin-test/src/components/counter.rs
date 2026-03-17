use orkester_plugin::sdk::{Component, Host, Message, OwnedMessage, Result};

pub struct CounterComponent {
    value: u64,
}

impl CounterComponent {
    pub fn new(initial: u64) -> Self {
        Self { value: initial }
    }
}

impl Component for CounterComponent {
    fn handle(&mut self, _host: Host, request: Message<'_>) -> Result<OwnedMessage> {
        let text = request.utf8()?;

        match text {
            "inc" => self.value += 1,
            "dec" => self.value = self.value.saturating_sub(1),
            "reset" => self.value = 0,
            _ => {}
        }

        Ok(OwnedMessage::new(
            request.id(),
            orkester_plugin::abi::TYPE_UTF8,
            orkester_plugin::abi::FLAG_RESPONSE,
            self.value.to_string().into_bytes(),
        ))
    }
}