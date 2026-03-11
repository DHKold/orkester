pub mod metrics;
pub mod rest;
pub mod state;
pub mod workflow;

use serde_json::Value;
use thiserror::Error;

// ── Error ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum ServerBuildError {
    #[error("Configuration error: {0}")]
    Config(String),
    #[error("Required dependency of type '{0}' is not available")]
    MissingDependency(String),
    #[error("Internal error: {0}")]
    Internal(String),
}

// ── Runtime handle ────────────────────────────────────────────────────────────

/// A running server instance, type-erased for the plugin and startup system.
///
/// Concrete implementations also implement their specific server trait
/// (e.g. [`state::StateServer`]).  Orkester's core can downcast via
/// [`AnyServer::as_any`] to obtain typed handles like
/// [`state::StateHandle`] for wiring dependent servers.
pub trait AnyServer: Send + Sync {
    fn name(&self) -> &str;
    /// Short type identifier matching the one declared in
    /// [`ServerFactory::server_type`] (e.g. `"state"`, `"metrics"`).
    fn server_type(&self) -> &str;
    /// Downcasting support so Orkester can obtain typed handles.
    fn as_any(&self) -> &dyn std::any::Any;
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;
}

// ── Factory ───────────────────────────────────────────────────────────────────

/// Generic factory trait implemented by every server a plugin contributes.
///
/// Orkester collects all registered factories from all loaded plugins,
/// topologically sorts them by [`ServerFactory::dependencies`], then builds
/// and starts them in order.
///
/// # Implementing a server
/// ```no_run
/// struct InMemoryStateFactory;
///
/// impl ServerFactory for InMemoryStateFactory {
///     fn server_type(&self) -> &str { "state" }
///     fn name(&self)        -> &str { "in-memory-state" }
///     fn dependencies(&self) -> Vec<String> { vec![] }
///     fn build(&self, config: Value) -> Result<Box<dyn AnyServer>, ServerBuildError> {
///         Ok(Box::new(InMemoryStateServer::new(config)?))
///     }
/// }
/// ```
pub trait ServerFactory: Send + Sync {
    /// Short, unique type identifier for this *kind* of server
    /// (e.g. `"state"`, `"workflow"`, `"metrics"`, `"rest"`).
    /// Used to resolve inter-server dependencies.
    fn server_type(&self) -> &str;

    /// Human-readable name of this specific implementation
    /// (e.g. `"memory-state"`, `"prometheus-metrics"`).
    fn name(&self) -> &str;

    /// Server type identifiers that must be built and running before this
    /// server's [`build`] is called.
    ///
    /// Example: a WorkflowServer that reads/writes state returns `vec!["state".into()]`.
    fn dependencies(&self) -> Vec<String> {
        vec![]
    }

    /// Build the server from the provided configuration.
    /// Called by Orkester after all declared dependencies are available.
    fn build(&self, config: Value) -> Result<Box<dyn AnyServer>, ServerBuildError>;
}

// ── Internal wiring helper (Orkester-core use only) ───────────────────────────

/// Communication channels and thread handle returned when a server starts.
pub struct ServerContext<Tx, Rx> {
    /// Receiver to read messages from the server
    pub receiver: Option<std::sync::mpsc::Receiver<Rx>>,
    /// Sender to send messages to the server
    pub sender: Option<std::sync::mpsc::Sender<Tx>>,
    /// Handle to the server thread; join on shutdown
    pub handle: std::thread::JoinHandle<()>,
}
