//! Workflows server — manages Crons, schedules, and Workflow execution.
//!
//! # Configuration
//!
//! ```yaml
//! servers:
//!   workflows:
//!     component:
//!       plugin: orkester-plugin-core
//!       server: workflows-server
//!     enabled: true
//!     rest_target: rest_api             # default: "rest_api"
//!     workspace_target: workspace       # default: "workspace"
//!     scheduler_interval_seconds: 30   # default: 30
//! ```
//!
//! # Architecture
//!
//! ```
//!  ┌──────────────────────────────────────────────────┐
//!  │  WorkflowsServer  (hub participant)              │
//!  │                                                  │
//!  │  ┌─────────────────────┐  ┌──────────────────┐  │
//!  │  │  Scheduler          │  │  ApiHandler       │  │
//!  │  │  (fires Crons,      │  │  (REST via hub)   │  │
//!  │  │   creates Workflows)│  └──────────────────-┘  │
//!  │  └───────┬─────────────┘                         │
//!  │          │ tokio::spawn per Workflow              │
//!  │  ┌───────▼─────────────┐                         │
//!  │  │  Worker             │                         │
//!  │  │  (drives execution) │                         │
//!  │  └─────────────────────┘                         │
//!  │                                                  │
//!  │  ┌──────────────────────┐                        │
//!  │  │  WorkflowsStore      │                        │
//!  │  │  (PersistenceProvider│                        │
//!  │  └──────────────────────┘                        │
//!  └──────────────────────────────────────────────────┘
//! ```

pub mod api;
pub mod model;
pub mod scheduler;
pub mod server;
pub mod store;
pub mod worker;
pub mod workspace_client;

use orkester_common::messaging::ServerSide;
use orkester_common::plugin::servers::{Server, ServerBuilder, ServerError};
use serde_json::Value;

// ── WorkflowsServer ───────────────────────────────────────────────────────────

pub struct WorkflowsServer {
    config: Value,
}

impl Server for WorkflowsServer {
    fn start(&self, channel: ServerSide) -> Result<(), ServerError> {
        let config = self.config.clone();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("Failed to build Tokio runtime");
            rt.block_on(server::run(config, channel));
        });
        Ok(())
    }

    fn stop(&self) -> Result<(), ServerError> {
        Ok(())
    }
}

// ── Builder ───────────────────────────────────────────────────────────────────

pub struct WorkflowsServerBuilder;

impl ServerBuilder for WorkflowsServerBuilder {
    fn build(&self, config: Value) -> Result<Box<dyn Server>, ServerError> {
        Ok(Box::new(WorkflowsServer { config }))
    }
}
