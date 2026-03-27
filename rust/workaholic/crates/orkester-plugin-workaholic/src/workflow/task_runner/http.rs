//! HTTP task runner (stub — not yet implemented).

use workaholic::{TaskRunnerDoc, TaskRunRequestDoc};

use super::traits::{TaskRun, TaskRunner, TaskRunnerError};

/// Executes tasks by sending an HTTP request and polling for completion.
/// Full implementation is pending; `spawn()` currently returns an error.
#[derive(Debug)]
pub struct HttpTaskRunner;

impl TaskRunner for HttpTaskRunner {
    fn as_doc(&self) -> TaskRunnerDoc {
        unimplemented!("HttpTaskRunner is not yet implemented")
    }

    fn spawn(
        &self,
        _request: TaskRunRequestDoc,
    ) -> Result<Box<dyn TaskRun>, TaskRunnerError> {
        Err(TaskRunnerError::UnsupportedKind(
            "http runner not yet implemented".to_string(),
        ))
    }
}