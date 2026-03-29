//! Orkester component wrapper for `ThreadWorkRunner`.

use serde::Deserialize;
use orkester_plugin::prelude::*;
use workaholic::{WorkRunDoc, WorkRunRequestDoc, WorkRunnerSpec, WorkaholicError};

use super::traits::WorkRunner;
use super::thread::ThreadWorkRunner;
use super::super::actions::*;

#[derive(Deserialize)]
pub struct ThreadWorkRunnerConfig {
    pub name:      String,
    pub namespace: String,
    pub spec:      WorkRunnerSpec,
}

pub struct ThreadWorkRunnerComponent {
    runner: ThreadWorkRunner,
}

#[component(
    kind        = "workaholic/ThreadWorkRunner:1.0",
    name        = "Thread Work Runner",
    description = "Orchestrates workflow runs on OS threads with DAG-based task scheduling.",
)]
impl ThreadWorkRunnerComponent {
    pub fn new(_host: *mut orkester_plugin::abi::AbiHost, config: ThreadWorkRunnerConfig) -> Self {
        Self { runner: ThreadWorkRunner::new(config.name, config.namespace, config.spec) }
    }

    #[handle(ACTION_WORK_RUNNER_GET)]
    fn get(&mut self, _: serde_json::Value) -> Result<serde_json::Value, WorkaholicError> {
        serde_json::to_value(self.runner.as_doc())
            .map_err(|e| WorkaholicError::SerializationError(e.to_string()))
    }

    #[handle(ACTION_WORK_RUNNER_SPAWN)]
    fn spawn(&mut self, request: WorkRunRequestDoc) -> Result<WorkRunDoc, WorkaholicError> {
        let run = self.runner.spawn(request)
            .map_err(|e| WorkaholicError::ExecutionError(e.to_string()))?;
        run.start().map_err(|e| WorkaholicError::ExecutionError(e.to_string()))?;
        Ok(run.as_doc())
    }
}
