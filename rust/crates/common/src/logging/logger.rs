use std::sync::atomic::{AtomicPtr, Ordering};
use std::sync::{OnceLock, RwLock};

use super::consumer::LogConsumer;
use super::level::Level;
use super::log::Log;

/// Pointer to a logger injected from the host process (e.g. into a cdylib plugin).
/// Non-null once `Logger::inject` has been called.
static INJECTED: AtomicPtr<Logger> = AtomicPtr::new(std::ptr::null_mut());

/// The process-owned global logger, created lazily when no injection is active.
static OWNED: OnceLock<Logger> = OnceLock::new();

fn global() -> &'static Logger {
    let ptr = INJECTED.load(Ordering::Acquire);
    if !ptr.is_null() {
        // SAFETY: `inject` requires the pointer to be valid for the process lifetime.
        unsafe { &*ptr }
    } else {
        OWNED.get_or_init(|| Logger::new(env!("CARGO_PKG_NAME")))
    }
}

// ── Logger ────────────────────────────────────────────────────────────────────

/// Holds a list of [`LogConsumer`]s and dispatches [`Log`] entries to them.
///
/// # Global logger
/// A process-wide instance is created lazily the first time it is accessed.
/// Use the static methods [`Logger::log`], [`Logger::add_consumer`], etc. to
/// interact with it without holding an explicit handle.
///
/// # Instance logger
/// Use [`Logger::new`] to create an independent logger (e.g. for a subsystem
/// or a test) with its own consumer list and source name.
pub struct Logger {
    source: String,
    consumers: RwLock<Vec<Box<dyn LogConsumer>>>,
}

impl Logger {
    /// Creates a new logger with the given default source and no consumers.
    pub fn new(source: impl Into<String>) -> Self {
        Self {
            source: source.into(),
            consumers: RwLock::new(Vec::new()),
        }
    }

    // ── Static API (global logger) ────────────────────────────────────────────

    /// Returns the global logger, creating it if necessary.
    pub fn global() -> &'static Logger {
        global()
    }

    /// Redirects every subsequent call to the global logger to `logger`.
    ///
    /// Plugins compiled as `cdylib` have their own static data segment and
    /// therefore a separate `OWNED` instance. Call this once — immediately after
    /// the plugin is loaded — to make all `log_*!` calls inside the plugin write
    /// to the host process's consumers instead.
    ///
    /// # Safety
    /// `logger` must remain valid for the entire remaining lifetime of the process
    /// (i.e. it should be `Logger::global()` from the host binary).
    pub unsafe fn inject(logger: *const Logger) {
        INJECTED.store(logger as *mut Logger, Ordering::Release);
    }

    /// Registers `consumer` with the global logger.
    pub fn add_consumer(consumer: impl LogConsumer + 'static) {
        global().register(Box::new(consumer));
    }

    /// Removes all consumers from the global logger.
    pub fn clear_consumers() {
        global()
            .consumers
            .write()
            .expect("logging consumer list poisoned")
            .clear();
    }

    /// Emits a log entry through the global logger.
    pub fn log(level: impl Into<Level>, message: impl Into<String>) {
        global().emit(level.into(), message.into());
    }

    /// Convenience shorthand for [`Logger::log`] at [`Level::TRACE`].
    pub fn trace(message: impl Into<String>) {
        Self::log(Level::TRACE, message);
    }

    /// Convenience shorthand for [`Logger::log`] at [`Level::DEBUG`].
    pub fn debug(message: impl Into<String>) {
        Self::log(Level::DEBUG, message);
    }

    /// Convenience shorthand for [`Logger::log`] at [`Level::INFO`].
    pub fn info(message: impl Into<String>) {
        Self::log(Level::INFO, message);
    }

    /// Convenience shorthand for [`Logger::log`] at [`Level::WARN`].
    pub fn warn(message: impl Into<String>) {
        Self::log(Level::WARN, message);
    }

    /// Convenience shorthand for [`Logger::log`] at [`Level::ERROR`].
    pub fn error(message: impl Into<String>) {
        Self::log(Level::ERROR, message);
    }

    // ── Instance API ──────────────────────────────────────────────────────────

    /// Adds a consumer to this logger instance.
    pub fn register(&self, consumer: Box<dyn LogConsumer>) {
        self.consumers
            .write()
            .expect("logging consumer list poisoned")
            .push(consumer);
    }

    /// Emits a log entry through this logger instance using its own source.
    pub fn emit(&self, level: impl Into<Level>, message: impl Into<String>) {
        let consumers = self
            .consumers
            .read()
            .expect("logging consumer list poisoned");
        if consumers.is_empty() {
            return;
        }
        let entry = Log::new(level.into(), &self.source, Vec::new(), message.into());
        for consumer in consumers.iter() {
            consumer.consume(&entry);
        }
    }

    /// Returns a [`ScopedLogger`] that overrides the source (and optionally tags)
    /// for a subset of log calls, while reusing this logger's consumer list.
    pub fn scoped(&self, source: impl Into<String>) -> ScopedLogger<'_> {
        ScopedLogger {
            inner: self,
            source: source.into(),
            tags: Vec::new(),
        }
    }
}

// ── ScopedLogger ──────────────────────────────────────────────────────────────

/// A temporary view over a [`Logger`] with an overridden `source` and optional
/// `tags`. Useful for attaching context without creating a new logger.
///
/// # Example
/// ```
/// Logger::global()
///     .scoped("auth")
///     .with_tag("request-id:abc123")
///     .log(Level::INFO, "user authenticated");
/// ```
pub struct ScopedLogger<'a> {
    inner: &'a Logger,
    source: String,
    tags: Vec<String>,
}

impl<'a> ScopedLogger<'a> {
    /// Adds a tag to this scoped view (builder-style, consumes self).
    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    /// Emits a log entry with this scope's source and tags.
    pub fn log(&self, level: impl Into<Level>, message: impl Into<String>) {
        let consumers = self
            .inner
            .consumers
            .read()
            .expect("logging consumer list poisoned");
        if consumers.is_empty() {
            return;
        }
        let entry = Log::new(
            level.into(),
            &self.source,
            self.tags.clone(),
            message.into(),
        );
        for consumer in consumers.iter() {
            consumer.consume(&entry);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::*;
    use crate::logging::{consumer::LogConsumer, level::Level, log::Log};

    /// Collects every received [`Log`] into a shared `Vec` for inspection.
    struct VecConsumer(Arc<Mutex<Vec<Log>>>);

    impl LogConsumer for VecConsumer {
        fn consume(&self, log: &Log) {
            self.0.lock().unwrap().push(log.clone());
        }
    }

    fn captured() -> (VecConsumer, Arc<Mutex<Vec<Log>>>) {
        let store = Arc::new(Mutex::new(Vec::new()));
        (VecConsumer(Arc::clone(&store)), store)
    }

    #[test]
    fn no_consumer_does_not_panic() {
        let logger = Logger::new("test");
        logger.emit(Level::INFO, "no consumer");
    }

    #[test]
    fn single_consumer_receives_log() {
        let logger = Logger::new("my-service");
        let (consumer, store) = captured();
        logger.register(Box::new(consumer));
        logger.emit(Level::INFO, "hello");

        let logs = store.lock().unwrap();
        assert_eq!(logs.len(), 1);
        assert_eq!(logs[0].message, "hello");
        assert_eq!(logs[0].level, Level::INFO);
        assert_eq!(logs[0].source, "my-service");
    }

    #[test]
    fn multiple_consumers_all_receive_every_log() {
        let logger = Logger::new("test");
        let (c1, s1) = captured();
        let (c2, s2) = captured();
        logger.register(Box::new(c1));
        logger.register(Box::new(c2));
        logger.emit(Level::WARN, "broadcast");

        assert_eq!(s1.lock().unwrap().len(), 1);
        assert_eq!(s2.lock().unwrap().len(), 1);
    }

    #[test]
    fn multiple_emissions_are_ordered() {
        let logger = Logger::new("test");
        let (consumer, store) = captured();
        logger.register(Box::new(consumer));
        logger.emit(Level::TRACE, "first");
        logger.emit(Level::ERROR, "second");

        let logs = store.lock().unwrap();
        assert_eq!(logs.len(), 2);
        assert_eq!(logs[0].message, "first");
        assert_eq!(logs[1].message, "second");
    }

    #[test]
    fn scoped_overrides_source() {
        let logger = Logger::new("root");
        let (consumer, store) = captured();
        logger.register(Box::new(consumer));
        logger
            .scoped("sub-module")
            .log(Level::INFO, "scoped message");

        let logs = store.lock().unwrap();
        assert_eq!(logs.len(), 1);
        assert_eq!(logs[0].source, "sub-module");
        assert_eq!(logs[0].message, "scoped message");
    }

    #[test]
    fn scoped_with_tags_attached() {
        let logger = Logger::new("root");
        let (consumer, store) = captured();
        logger.register(Box::new(consumer));
        logger
            .scoped("auth")
            .with_tag("req-id:abc")
            .with_tag("user:alice")
            .log(Level::INFO, "authenticated");

        let logs = store.lock().unwrap();
        assert_eq!(logs.len(), 1);
        assert_eq!(logs[0].tags, vec!["req-id:abc", "user:alice"]);
        assert_eq!(logs[0].source, "auth");
    }

    #[test]
    fn custom_numeric_level_is_forwarded() {
        let logger = Logger::new("test");
        let (consumer, store) = captured();
        logger.register(Box::new(consumer));
        logger.emit(Level(99), "critical-custom");

        let logs = store.lock().unwrap();
        assert_eq!(logs[0].level, Level(99));
    }

    #[test]
    fn scoped_with_no_tags_has_empty_tags() {
        let logger = Logger::new("root");
        let (consumer, store) = captured();
        logger.register(Box::new(consumer));
        logger.scoped("svc").log(Level::DEBUG, "no tags");

        let logs = store.lock().unwrap();
        assert!(logs[0].tags.is_empty());
    }
}
