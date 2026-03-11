//! Logging module.
//!
//! # Quick start
//!
//! ```no_run
//! use crate::logging::{Logger, Level, consumers::ConsoleConsumer};
//!
//! // Register one or more consumers once at startup.
//! Logger::add_consumer(ConsoleConsumer);
//!
//! // Static helpers — source defaults to the crate name.
//! Logger::log(Level::INFO, "hello");
//! Logger::info("also works");
//!
//! // Macros — source is set to the call-site module path automatically.
//! log_info!("server started on port {}", 8080);
//! log_warn!("retrying in {} ms", delay);
//!
//! // Manual scoping — attach an explicit source and/or tags.
//! Logger::global()
//!     .scoped("auth")
//!     .with_tag("user:alice")
//!     .log(Level::DEBUG, "session created");
//! ```

pub mod consumer;
pub mod consumers;
pub mod level;
pub mod log;
pub mod logger;
pub mod macros;

pub use consumer::LogConsumer;
pub use level::Level;
pub use log::Log;
pub use logger::{Logger, ScopedLogger};

