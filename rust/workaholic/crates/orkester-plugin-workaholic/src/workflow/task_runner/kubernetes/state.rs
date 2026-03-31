use workaholic::TaskRunState;

/// Mutable state shared between a `KubernetesTaskRun` and its execution thread.
#[derive(Debug)]
pub struct KubeTaskRunState {
    pub run_state:        TaskRunState,
    pub cancel_requested: bool,
    pub job_name:         Option<String>,
    pub stdout:           String,
}

impl KubeTaskRunState {
    pub fn pending() -> Self {
        Self {
            run_state:        TaskRunState::Pending,
            cancel_requested: false,
            job_name:         None,
            stdout:           String::new(),
        }
    }
}
