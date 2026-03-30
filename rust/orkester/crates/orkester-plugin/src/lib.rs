pub mod abi;
pub mod hub;
pub mod logging;
pub mod sdk;
pub mod prelude;

// Re-export the proc macro so users can write `use orkester_plugin::prelude::*`
// and get `#[component]` in scope without a separate `extern crate orkester_macro`.
pub use orkester_macro::component;

/// Generate the C entry point for a plugin whose root component implements
/// [`sdk::PluginComponent`] and [`Default`].
///
/// ```ignore
/// orkester_plugin::export_plugin_root!(my_crate::RootComponent);
/// ```
#[macro_export]
macro_rules! export_plugin_root {
    ($ty:ty) => {
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn orkester_plugin_entry(
            host: *mut $crate::abi::AbiHost,
        ) -> *mut $crate::abi::AbiComponent {
            $crate::logging::init_logging(host, env!("CARGO_PKG_NAME"));
            use $crate::sdk::PluginComponent as _;
            let component = <$ty as ::std::default::Default>::default();
            ::std::boxed::Box::into_raw(::std::boxed::Box::new(
                <$ty as $crate::sdk::PluginComponent>::to_abi(component),
            ))
        }
    };
}

/// Like [`export_plugin_root!`] but passes the raw `*mut AbiHost` to
/// `<Type>::new(host)` instead of calling `Default::default()`.
///
/// The root component type must expose:
/// ```ignore
/// fn new(host: *mut orkester_plugin::abi::AbiHost) -> Self
/// ```
///
/// Use this variant when any child component created by the root needs to call
/// back to the host (e.g. a RestServer routing through the host dispatcher).
///
/// ```ignore
/// orkester_plugin::export_plugin_root_with_host!(my_crate::RootComponent);
/// ```
#[macro_export]
macro_rules! export_plugin_root_with_host {
    ($ty:ty) => {
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn orkester_plugin_entry(
            host: *mut $crate::abi::AbiHost,
        ) -> *mut $crate::abi::AbiComponent {
            $crate::logging::init_logging(host, env!("CARGO_PKG_NAME"));
            use $crate::sdk::PluginComponent as _;
            let component = <$ty>::new(host);
            ::std::boxed::Box::into_raw(::std::boxed::Box::new(
                <$ty as $crate::sdk::PluginComponent>::to_abi(component),
            ))
        }
    };
}

// ─── Logging macros ───────────────────────────────────────────────────────────

/// Log at TRACE level.  Arguments are identical to `format!`.
#[macro_export]
macro_rules! log_trace {
    ($($arg:tt)*) => {
        $crate::logging::send_log($crate::logging::LogRecord {
            level:        $crate::logging::LogLevel::Trace,
            target:       module_path!().to_owned(),
            message:      format!($($arg)*),
            file:         file!().to_owned(),
            line:         line!(),
            timestamp_ms: $crate::logging::now_ms(),
            plugin_id:    $crate::logging::plugin_id().to_owned(),
        })
    };
}

/// Log at DEBUG level.  Arguments are identical to `format!`.
#[macro_export]
macro_rules! log_debug {
    ($($arg:tt)*) => {
        $crate::logging::send_log($crate::logging::LogRecord {
            level:        $crate::logging::LogLevel::Debug,
            target:       module_path!().to_owned(),
            message:      format!($($arg)*),
            file:         file!().to_owned(),
            line:         line!(),
            timestamp_ms: $crate::logging::now_ms(),
            plugin_id:    $crate::logging::plugin_id().to_owned(),
        })
    };
}

/// Log at INFO level.  Arguments are identical to `format!`.
#[macro_export]
macro_rules! log_info {
    ($($arg:tt)*) => {
        $crate::logging::send_log($crate::logging::LogRecord {
            level:        $crate::logging::LogLevel::Info,
            target:       module_path!().to_owned(),
            message:      format!($($arg)*),
            file:         file!().to_owned(),
            line:         line!(),
            timestamp_ms: $crate::logging::now_ms(),
            plugin_id:    $crate::logging::plugin_id().to_owned(),
        })
    };
}

/// Log at WARN level.  Arguments are identical to `format!`.
#[macro_export]
macro_rules! log_warn {
    ($($arg:tt)*) => {
        $crate::logging::send_log($crate::logging::LogRecord {
            level:        $crate::logging::LogLevel::Warn,
            target:       module_path!().to_owned(),
            message:      format!($($arg)*),
            file:         file!().to_owned(),
            line:         line!(),
            timestamp_ms: $crate::logging::now_ms(),
            plugin_id:    $crate::logging::plugin_id().to_owned(),
        })
    };
}

/// Log at ERROR level.  Arguments are identical to `format!`.
#[macro_export]
macro_rules! log_error {
    ($($arg:tt)*) => {
        $crate::logging::send_log($crate::logging::LogRecord {
            level:        $crate::logging::LogLevel::Error,
            target:       module_path!().to_owned(),
            message:      format!($($arg)*),
            file:         file!().to_owned(),
            line:         line!(),
            timestamp_ms: $crate::logging::now_ms(),
            plugin_id:    $crate::logging::plugin_id().to_owned(),
        })
    };
}
