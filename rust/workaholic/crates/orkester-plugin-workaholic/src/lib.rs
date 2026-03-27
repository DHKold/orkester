//! Workaholic workflow execution engine — Orkester plugin.
//!
//! This crate contains:
//! - `workflow` — runtime traits, events, and implementations for `WorkRunner`,
//!   `WorkRun`, `TaskRunner`, and `TaskRun`.
//! - `catalog` — the catalog server component.
//! - `document` — document loaders, parsers, and persisters.

pub mod catalog;
pub mod document;
pub mod workflow;

pub use workflow::{
    ContainerTaskRunner, HttpTaskRunner, ShellTaskRunner, ThreadWorkRunner,
    TaskRun, TaskRunError, TaskRunEvent, TaskRunEventStream, TaskRunner, TaskRunnerError,
    WorkRun, WorkRunError, WorkRunEvent, WorkRunEventStream, WorkRunResources,
    WorkRunner, WorkRunnerError,
};

// Create the root component and export the plugin.
use orkester_plugin::prelude::*;

use workaholic::WorkaholicError;

orkester_plugin::export_plugin_root_with_host!(RootComponent);

pub struct RootComponent {
    host_ptr: *mut orkester_plugin::abi::AbiHost,
}

unsafe impl Send for RootComponent {}

#[component(
    kind = "workaholic/Root:1.0.0",
    name = "Workaholic Root Component",
    description = "Root component for the Workaholic plugin, providing workflow execution capabilities."
)]
impl RootComponent {
    fn new(host_ptr: *mut orkester_plugin::abi::AbiHost) -> Self {
        Self { host_ptr }
    }

    #[factory("workaholic/CatalogServer:1.0")]
    fn create_catalog_server(&mut self, config: catalog::CatalogServerConfig) -> Result<catalog::CatalogServer, WorkaholicError> {
        Ok(catalog::CatalogServer::new(self.host_ptr, config))
    }
}