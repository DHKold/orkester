use orkester_plugin::sdk::{Component, Host, Message, OwnedMessage, Result};
use orkester_plugin::sdk::protocol::ComponentMetadata;

pub struct CounterComponent {
    value: u64,
}

impl CounterComponent {
    pub fn new(initial: u64) -> Self {
        Self { value: initial }
    }

    pub fn metadata() -> ComponentMetadata {
        ComponentMetadata {
            id: "counter".to_string(),
            name: Some("Counter Component".to_string()),
            description: Some("A simple counter component that can be incremented, decremented, or reset.".to_string()),
            input_types: vec![orkester_plugin::sdk::protocol::constants::MSG_TYPE_STRING],
            output_types: vec![orkester_plugin::sdk::protocol::constants::MSG_TYPE_STRING],
            extra: serde_json::Map::new(),
        }
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
            orkester_plugin::sdk::protocol::constants::MSG_TYPE_STRING,
            0,
            self.value.to_string().into_bytes(),
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
    fn counter_basic_ops() {
        let mut comp = CounterComponent::new(5);
        let host = Host::new(ptr::null());

        let inc = Message::new(1, TYPE_UTF8, 0, b"inc");
        let res = comp.handle(host, inc).expect("inc failed");
        assert_eq!(res.utf8().unwrap(), "6");

        let dec = Message::new(2, TYPE_UTF8, 0, b"dec");
        let res = comp.handle(host, dec).expect("dec failed");
        assert_eq!(res.utf8().unwrap(), "5");

        let reset = Message::new(3, TYPE_UTF8, 0, b"reset");
        let res = comp.handle(host, reset).expect("reset failed");
        assert_eq!(res.utf8().unwrap(), "0");
    }
}