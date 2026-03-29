//! Orkester component wrapper for `ContainerTaskRunner`.

use serde::Deserialize;
use orkester_plugin::prelude::*;
use workaholic::{TaskRunDoc, TaskRunRequestDoc, TaskRunnerSpec, WorkaholicError};

use super::traits::TaskRunner;
use super::container::ContainerTaskRunner;
use super::super::actions::*;

#[derive(Deserialize)]
pub struct ContainerTaskRunnerConfig {
    pub name:      String,
    pub namespace: String,
    pub spec:      TaskRunnerSpec,
}

pub struct ContainerTaskRunnerComponent {
    runner: ContainerTaskRunner,
}

#[component(
    kind        = "workaholic/ContainerTaskRunner:1.0",
    name        = "Container Task Runner",
    description = "Executes tasks inside OCI containers via the Docker or Podman CLI.",
)]
impl ContainerTaskRunnerComponent {
    pub fn new(_host: *mut orkester_plugin::abi::AbiHost, config: ContainerTaskRunnerConfig) -> Self {
        Self { runner: ContainerTaskRunner::new(config.name, config.namespace, config.spec) }
    }

    #[handle(ACTION_TASK_RUNNER_GET)]
    fn get(&mut self, _: serde_json::Value) -> Result<serde_json::Value, WorkaholicError> {
        serde_json::to_value(self.runner.as_doc()).map_err(|e| WorkaholicError::SerializationError(e.to_string()))
    }

    #[handle(ACTION_TASK_RUNNER_SPAWN)]
    fn spawn(&mut self, request: TaskRunRequestDoc) -> Result<TaskRunDoc, WorkaholicError> {
        let run = self.runner.spawn(request)
            .map_err(|e| WorkaholicError::ExecutionError(e.to_string()))?;
        run.start().map_err(|e| WorkaholicError::ExecutionError(e.to_string()))?;
        Ok(run.as_doc())
    }
}
