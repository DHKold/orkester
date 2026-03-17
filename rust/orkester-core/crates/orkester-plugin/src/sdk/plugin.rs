use super::{Component, Host, Message, OwnedMessage, Result};

pub trait Plugin: Sized + 'static {
    fn new(host: Host) -> Result<Self>;
    fn handle(&mut self, request: Message<'_>) -> Result<OwnedMessage>;
    fn create_component(&mut self, request: Message<'_>) -> Result<Box<dyn Component>>;
}