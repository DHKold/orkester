use super::Dispatcher;
use crate::hub::{envelope::Envelope, error::DispatchError};
use crate::{log_trace};

/// Silently discards every envelope it receives.
///
/// Useful as a catch-all rule of last resort to prevent the hub from logging
/// "unrouted" warnings for known-unimportant traffic.
pub struct DropDispatcher;

impl Dispatcher for DropDispatcher {
    fn name(&self) -> &str { "drop" }

    fn dispatch(&self, envelope: Envelope) -> Result<Vec<Envelope>, DispatchError> {
        log_trace!("[hub/drop] id={} kind='{}' dropped", envelope.id, envelope.kind);
        Ok(Vec::new())
    }
}
