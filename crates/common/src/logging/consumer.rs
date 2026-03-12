use super::log::Log;
use super::filter::LogFilter;

/// Receives a [`Log`] entry and does something with it — prints it, forwards it,
/// persists it, etc.
///
/// Implement this trait to plug custom sinks into a [`Logger`].
/// Consumers must be `Send + Sync` so they can be used from multiple threads
/// through the shared global logger.
pub trait LogConsumer: Send + Sync {
    fn consume(&self, log: &Log);
    fn set_filter(&self, filter: Option<Box<dyn LogFilter + 'static>>);
}
