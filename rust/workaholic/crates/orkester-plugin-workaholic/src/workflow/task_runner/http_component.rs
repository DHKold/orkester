//! Orkester component wrapper for `HttpTaskRunner`.

use serde::Deserialize;
use orkester_plugin::prelude::*;
use workaholic::{TaskRunDoc, TaskRunRequestDoc, TaskRunnerSpec, WorkaholicError};

use super::traits::TaskRunner;
use super::http::HttpTaskRunner;
use super::super::actions::*;

#[derive(Deserialize)]
pub struct HttpTaskRunnerConfig {
    pub name:      String,
    pub namespace: String,
    pub spec:      TaskRunnerSpec,
}

pub struct HttpTaskRunnerComponent {
    runner: HttpTaskRunner,
}

#[component(
    kind        = "workaholic/HttpTaskRunner:1.0",
    name        = "HTTP Task Runner",
    description = "Executes tasks via HTTP POST and polls for job completion.",
)]
impl HttpTaskRunnerComponent {
    pub fn new(_host: *mut orkester_plugin::abi::AbiHost, config: HttpTaskRunnerConfig) -> Self {
        Self { runner: HttpTaskRunner::new(config.name, config.namespace, config.spec) }
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
