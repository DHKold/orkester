use std::sync::{Arc, Mutex};

use kube::Client;
use orkester_plugin::log_error;
use workaholic::TaskRunState;

use super::config::KubeJobConfig;
use super::job::{create_job, delete_job, fetch_logs, wait_for_completion};
use super::state::KubeTaskRunState;
use crate::workflow::task_runner::traits::TaskRunEvent;

async fn execute(
    cfg:      &KubeJobConfig,
    job_name: &str,
    state:    &Arc<Mutex<KubeTaskRunState>>,
) -> (TaskRunState, String) {
    let client = match Client::try_default().await {
        Ok(c)  => c,
        Err(e) => {
            log_error!("[k8s] kube client error: {}", e);
            return (TaskRunState::Failed, String::new());
        }
    };
    if let Err(e) = create_job(&client, cfg, job_name).await {
        log_error!("[k8s] failed to create job '{}': {}", job_name, e);
        return (TaskRunState::Failed, String::new());
    }
    state.lock().unwrap().job_name = Some(job_name.to_string());
    let outcome = wait_for_completion(&client, cfg, job_name, state).await;

    // When the cancel path already initiated deletion via `spawn_cancel_deletion`,
    // skip fetching logs and deleting again (the job may already be terminating).
    if state.lock().unwrap().deletion_initiated {
        return (outcome, String::new());
    }

    let stdout = fetch_logs(&client, &cfg.namespace, job_name).await;
    delete_job(&client, &cfg.namespace, job_name).await;
    (outcome, stdout)
}

fn finish(
    state:     &Arc<Mutex<KubeTaskRunState>>,
    sender:    &crossbeam_channel::Sender<TaskRunEvent>,
    new_state: TaskRunState,
) {
    state.lock().unwrap().run_state = new_state.clone();
    let _ = sender.send(TaskRunEvent::StateChanged(new_state));
    let _ = sender.send(TaskRunEvent::Finished);
}

pub fn run_kubernetes_job(
    cfg:      KubeJobConfig,
    job_name: String,
    state:    Arc<Mutex<KubeTaskRunState>>,
    sender:   crossbeam_channel::Sender<TaskRunEvent>,
) {
    if state.lock().unwrap().cancel_requested {
        let _ = sender.send(TaskRunEvent::Finished);
        return;
    }
    let rt = match tokio::runtime::Builder::new_current_thread().enable_all().build() {
        Ok(rt) => rt,
        Err(e) => {
            log_error!("[k8s] failed to build tokio runtime: {}", e);
            finish(&state, &sender, TaskRunState::Failed);
            return;
        }
    };
    let (outcome, stdout) = rt.block_on(execute(&cfg, &job_name, &state));
    state.lock().unwrap().stdout = stdout;
    finish(&state, &sender, outcome);
}
