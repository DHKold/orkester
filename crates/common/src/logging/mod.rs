//! Logging module.
//!
//! # Quick start
//!
//! ```no_run
//! use orkester_common::logging::{Logger, Level, consumers::{ConsoleConsumer, MinLevel}};
//!
//! // Register one or more consumers once at startup.
//! let consumer = ConsoleConsumer::new();
//! consumer.set_filter(Some(MinLevel::new(Level::INFO)));
//! Logger::add_consumer(consumer);
//!
//! // Static helpers — source defaults to the crate name.
//! Logger::log(Level::INFO, "hello");
//! Logger::info("also works");
//!
//! // Macros — source is set to the call-site module path automatically.
//! log_info!("server started on port {}", 8080);
//! log_warn!("retrying in {} ms", delay);
//! ```

pub mod consumer;
pub mod consumers;
pub mod filter;
pub mod level;
pub mod log;
pub mod logger;
pub mod macros;

pub use consumer::LogConsumer;
pub use filter::LogFilter;
pub use level::Level;
pub use log::Log;
pub use logger::{Logger, ScopedLogger};
