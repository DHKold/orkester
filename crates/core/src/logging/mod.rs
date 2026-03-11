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
//! // Log from anywhere — no handle needed.
//! Logger::log(Level::INFO, "hello");
//! Logger::info("also works");
//!
//! // Attach a source / tags for a call site.
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

pub use consumer::LogConsumer;
pub use level::Level;
pub use log::Log;
pub use logger::{Logger, ScopedLogger};

