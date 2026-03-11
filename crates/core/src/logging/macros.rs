/// Log a message at an arbitrary level through the global logger.
///
/// The `source` field of the resulting [`Log`] is automatically set to the
/// Rust module path of the *call site* (e.g. `"orkester::server"`), so you
/// never have to pass it manually.
///
/// # Examples
/// ```
/// log!(Level::INFO, "plain message");
/// log!(Level::WARN, "value is {}", my_value);
/// ```
#[macro_export]
macro_rules! log {
    ($level:expr, $($arg:tt)*) => {
        $crate::logging::Logger::global()
            .scoped(module_path!())
            .log($level, format!($($arg)*))
    };
}

/// Log at [`Level::TRACE`] with automatic call-site source.
#[macro_export]
macro_rules! log_trace {
    ($($arg:tt)*) => {
        $crate::log!($crate::logging::Level::TRACE, $($arg)*)
    };
}

/// Log at [`Level::DEBUG`] with automatic call-site source.
#[macro_export]
macro_rules! log_debug {
    ($($arg:tt)*) => {
        $crate::log!($crate::logging::Level::DEBUG, $($arg)*)
    };
}

/// Log at [`Level::INFO`] with automatic call-site source.
#[macro_export]
macro_rules! log_info {
    ($($arg:tt)*) => {
        $crate::log!($crate::logging::Level::INFO, $($arg)*)
    };
}

/// Log at [`Level::WARN`] with automatic call-site source.
#[macro_export]
macro_rules! log_warn {
    ($($arg:tt)*) => {
        $crate::log!($crate::logging::Level::WARN, $($arg)*)
    };
}

/// Log at [`Level::ERROR`] with automatic call-site source.
#[macro_export]
macro_rules! log_error {
    ($($arg:tt)*) => {
        $crate::log!($crate::logging::Level::ERROR, $($arg)*)
    };
}
