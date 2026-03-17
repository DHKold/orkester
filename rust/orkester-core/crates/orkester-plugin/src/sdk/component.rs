use super::{Host, Message, OwnedMessage, Result};

pub trait Component: Send + 'static {
    fn handle(&mut self, host: Host, request: Message<'_>) -> Result<OwnedMessage>;
}