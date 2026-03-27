//! Container-based task runner (stub — not yet implemented).

use workaholic::{TaskRunnerDoc, TaskRunRequestDoc};

use super::traits::{TaskRun, TaskRunner, TaskRunnerError};

/// Executes tasks inside an OCI container.
/// Full implementation is pending; `spawn()` currently returns an error.
#[derive(Debug)]
pub struct ContainerTaskRunner;

impl TaskRunner for ContainerTaskRunner {
    fn as_doc(&self) -> TaskRunnerDoc {
        unimplemented!("ContainerTaskRunner is not yet implemented")
    }

    fn spawn(
        &self,
        _request: TaskRunRequestDoc,
    ) -> Result<Box<dyn TaskRun>, TaskRunnerError> {
        Err(TaskRunnerError::UnsupportedKind(
            "container runner not yet implemented".to_string(),
        ))
    }
}