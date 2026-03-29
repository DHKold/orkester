//! Orkester component wrapper for `ShellTaskRunner`.

use serde::Deserialize;
use orkester_plugin::prelude::*;
use workaholic::{TaskRunDoc, TaskRunRequestDoc, TaskRunnerSpec, WorkaholicError};

use super::traits::TaskRunner;
use super::shell::ShellTaskRunner;
use super::super::actions::*;

#[derive(Deserialize)]
pub struct ShellTaskRunnerConfig {
    pub name:      String,
    pub namespace: String,
    pub spec:      TaskRunnerSpec,
}

pub struct ShellTaskRunnerComponent {
    runner: ShellTaskRunner,
}

#[component(
    kind        = "workaholic/ShellTaskRunner:1.0",
    name        = "Shell Task Runner",
    description = "Executes tasks by running shell scripts in child processes.",
)]
impl ShellTaskRunnerComponent {
    pub fn new(_host: *mut orkester_plugin::abi::AbiHost, config: ShellTaskRunnerConfig) -> Self {
        Self { runner: ShellTaskRunner::new(config.name, config.namespace, config.spec) }
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
