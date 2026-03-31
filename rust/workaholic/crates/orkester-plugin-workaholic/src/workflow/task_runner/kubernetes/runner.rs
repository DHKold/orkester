use std::sync::Mutex;

use uuid::Uuid;
use workaholic::{
    DocumentMetadata, TaskRunRequestDoc, TaskRunnerDoc, TaskRunnerSpec,
    TaskRunnerState, TaskRunnerStatus, TASK_RUNNER_KIND,
};

use super::super::traits::{TaskRun, TaskRunnerError, TaskRunner};
use super::config::parse_config;
use super::run::KubernetesTaskRun;

#[derive(Debug)]
pub struct KubernetesTaskRunner {
    name:      String,
    namespace: String,
    spec:      TaskRunnerSpec,
    state:     Mutex<TaskRunnerState>,
}

impl KubernetesTaskRunner {
    pub fn new(
        name:      impl Into<String>,
        namespace: impl Into<String>,
        spec:      TaskRunnerSpec,
    ) -> Self {
        Self {
            name:      name.into(),
            namespace: namespace.into(),
            spec,
            state: Mutex::new(TaskRunnerState::Ready),
        }
    }

    fn self_ref(&self) -> String {
        format!("worker://{}/{}:1.0.0", self.namespace, self.name)
    }
}

impl TaskRunner for KubernetesTaskRunner {
    fn as_doc(&self) -> TaskRunnerDoc {
        let state = self.state.lock().unwrap();
        TaskRunnerDoc {
            kind: TASK_RUNNER_KIND.to_string(),
            name: self.name.clone(),
            version: "1.0.0".to_string(),
            metadata: DocumentMetadata {
                namespace: Some(self.namespace.clone()),
                owner: None, description: None, tags: vec![], extra: Default::default(),
            },
            spec: self.spec.clone(),
            status: Some(TaskRunnerStatus {
                state: state.clone(), metrics: Default::default(), state_history: vec![],
            }),
        }
    }

    fn spawn(&self, request: TaskRunRequestDoc) -> Result<Box<dyn TaskRun>, TaskRunnerError> {
        let cfg = parse_config(&request).map_err(TaskRunnerError::Other)?;
        let run = KubernetesTaskRun::new(
            Uuid::new_v4().to_string(),
            self.namespace.clone(),
            self.self_ref(),
            request,
            cfg,
        );
        Ok(Box::new(run))
    }
}
