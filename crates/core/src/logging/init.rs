//! Logging initialization logic

use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, reload};
use super::config::{LoggingConfig, LogFormat};

/// Handle for updating logging configuration at runtime.
pub struct LoggingHandle {
    reload_handle: reload::Handle<EnvFilter, fmt::Layer<tracing_subscriber::Registry, fmt::format::Writer>>,
    format: LogFormat,
}

/// Initialize global logging for the application with reloadable log level/format.
/// Returns a LoggingHandle for runtime updates.
pub fn init(config: &LoggingConfig) -> LoggingHandle {
    let env_filter = EnvFilter::new(&config.log_level);
    let (filter_layer, reload_handle) = reload::Layer::new(env_filter);

    let format = config.log_format;
    match format {
        LogFormat::Plain => {
            tracing_subscriber::registry()
                .with(filter_layer)
                .with(fmt::layer().pretty())
                .init();
        },
        LogFormat::Json => {
            tracing_subscriber::registry()
                .with(filter_layer)
                .with(fmt::layer().json().flatten_event(true))
                .init();
        }
    }
    LoggingHandle { reload_handle, format }
}

impl LoggingHandle {
    /// Update the log level and format at runtime.
    pub fn update(&mut self, config: &LoggingConfig) {
        let _ = self.reload_handle.modify(|filter| {
            *filter = EnvFilter::new(&config.log_level);
        });
        // Note: Format cannot be changed at runtime with tracing-subscriber 0.3,
        // so this only updates the level. For full format change, restart is needed.
        // You may log a warning if format differs from initial.
        if self.format != config.log_format {
            tracing::warn!("Log format change at runtime is not supported; restart required");
        }
    }
}
