/// Core domain types: Workspace, Work, Task, Artifact, Execution, etc.
pub mod domain;

/// Logging module — [`Logger`], [`Level`], consumers and macros.
pub mod logging;

/// Messaging contract: [`Message`] and [`ServerSide`] channel passed to servers on start.
pub mod messaging;

/// Plugin registration types and the plugin loading contract.
pub mod plugin;
