use super::Dispatcher;
use crate::hub::{envelope::Envelope, error::DispatchError};

/// Silently discards every envelope it receives.
///
/// Useful as a catch-all rule of last resort to prevent the hub from logging
/// "unrouted" warnings for known-unimportant traffic.
pub struct DropDispatcher;

impl Dispatcher for DropDispatcher {
    fn name(&self) -> &str { "drop" }

    fn dispatch(&self, envelope: Envelope) -> Result<Vec<Envelope>, DispatchError> {
        log::debug!("[hub/drop] id={} kind='{}' dropped",envelope.id, envelope.kind);
        Ok(Vec::new())
    }
}
