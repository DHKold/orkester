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
    ContainerTaskRunner, HttpTaskRunner, KubernetesTaskRunner, ShellTaskRunner, ThreadWorkRunner,
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

    #[factory("workaholic/LocalFsLoader:1.0")]
    fn create_local_fs_loader(&mut self, config: document::loader::local_fs::LocalFsLoaderConfig) -> Result<document::loader::local_fs::LocalFsLoaderComponent, WorkaholicError> {
        Ok(document::loader::local_fs::LocalFsLoaderComponent::new(self.host_ptr, config))
    }

    #[factory("workaholic/ShellTaskRunner:1.0")]
    fn create_shell_task_runner(&mut self, config: workflow::ShellTaskRunnerConfig) -> Result<workflow::ShellTaskRunnerComponent, WorkaholicError> {
        Ok(workflow::ShellTaskRunnerComponent::new(self.host_ptr, config))
    }

    #[factory("workaholic/HttpTaskRunner:1.0")]
    fn create_http_task_runner(&mut self, config: workflow::HttpTaskRunnerConfig) -> Result<workflow::HttpTaskRunnerComponent, WorkaholicError> {
        Ok(workflow::HttpTaskRunnerComponent::new(self.host_ptr, config))
    }

    #[factory("workaholic/ContainerTaskRunner:1.0")]
    fn create_container_task_runner(&mut self, config: workflow::ContainerTaskRunnerConfig) -> Result<workflow::ContainerTaskRunnerComponent, WorkaholicError> {
        Ok(workflow::ContainerTaskRunnerComponent::new(self.host_ptr, config))
    }

    #[factory("workaholic/KubernetesTaskRunner:1.0")]
    fn create_kubernetes_task_runner(&mut self, config: workflow::KubernetesTaskRunnerConfig) -> Result<workflow::KubernetesTaskRunnerComponent, WorkaholicError> {
        Ok(workflow::KubernetesTaskRunnerComponent::new(self.host_ptr, config))
    }

    #[factory("workaholic/ThreadWorkRunner:1.0")]
    fn create_thread_work_runner(&mut self, config: workflow::ThreadWorkRunnerConfig) -> Result<workflow::ThreadWorkRunnerComponent, WorkaholicError> {
        Ok(workflow::ThreadWorkRunnerComponent::new(self.host_ptr, config))
    }

    #[factory("workaholic/WorkflowServer:1.0")]
    fn create_workflow_server(&mut self, config: workflow::WorkflowServerConfig) -> Result<workflow::WorkflowServerComponent, WorkaholicError> {
        Ok(workflow::WorkflowServerComponent::new(self.host_ptr, config))
    }
}