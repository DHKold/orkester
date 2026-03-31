use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use k8s_openapi::api::batch::v1::{Job, JobSpec};
use k8s_openapi::api::core::v1::{Container, EnvVar, Pod, PodSpec, PodTemplateSpec};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;
use kube::api::{Api, DeleteParams, ListParams, LogParams, PostParams};
use kube::Client;
use orkester_plugin::log_warn;
use workaholic::TaskRunState;

use super::config::KubeJobConfig;
use super::state::KubeTaskRunState;

fn build_container(cfg: &KubeJobConfig, env: Vec<EnvVar>) -> Container {
    Container {
        name: "task".to_string(),
        image: Some(cfg.image.clone()),
        command: if cfg.command.is_empty() { None } else { Some(cfg.command.clone()) },
        args: if cfg.args.is_empty() { None } else { Some(cfg.args.clone()) },
        env: if env.is_empty() { None } else { Some(env) },
        ..Default::default()
    }
}

fn build_job(job_name: &str, cfg: &KubeJobConfig, container: Container) -> Job {
    Job {
        metadata: ObjectMeta {
            name: Some(job_name.to_string()),
            namespace: Some(cfg.namespace.clone()),
            ..Default::default()
        },
        spec: Some(JobSpec {
            backoff_limit: Some(0),
            template: PodTemplateSpec {
                spec: Some(PodSpec {
                    restart_policy: Some("Never".to_string()),
                    service_account_name: cfg.service_account.clone(),
                    containers: vec![container],
                    ..Default::default()
                }),
                ..Default::default()
            },
            ..Default::default()
        }),
        ..Default::default()
    }
}

pub async fn create_job(client: &Client, cfg: &KubeJobConfig, job_name: &str) -> Result<(), kube::Error> {
    let env: Vec<EnvVar> = cfg.env_vars.iter()
        .map(|(k, v)| EnvVar { name: k.clone(), value: Some(v.clone()), ..Default::default() })
        .collect();
    let job = build_job(job_name, cfg, build_container(cfg, env));
    Api::<Job>::namespaced(client.clone(), &cfg.namespace)
        .create(&PostParams::default(), &job)
        .await?;
    Ok(())
}

pub async fn wait_for_completion(
    client:   &Client,
    cfg:      &KubeJobConfig,
    job_name: &str,
    state:    &Arc<Mutex<KubeTaskRunState>>,
) -> TaskRunState {
    let jobs: Api<Job> = Api::namespaced(client.clone(), &cfg.namespace);
    let deadline = Instant::now() + Duration::from_secs(cfg.timeout_secs);
    loop {
        if state.lock().unwrap().cancel_requested { return TaskRunState::Cancelled; }
        if Instant::now() >= deadline {
            log_warn!("[k8s] job '{}' timed out", job_name);
            return TaskRunState::Failed;
        }
        match jobs.get(job_name).await {
            Ok(job) => {
                if let Some(s) = job.status {
                    if s.succeeded.unwrap_or(0) > 0 { return TaskRunState::Succeeded; }
                    if s.failed.unwrap_or(0) > 0    { return TaskRunState::Failed; }
                }
            }
            Err(e) => log_warn!("[k8s] status query error: {}", e),
        }
        tokio::time::sleep(Duration::from_secs(cfg.poll_secs)).await;
    }
}

pub async fn fetch_logs(client: &Client, namespace: &str, job_name: &str) -> String {
    let pods: Api<Pod> = Api::namespaced(client.clone(), namespace);
    let selector = format!("job-name={job_name}");
    let list = match pods.list(&ListParams::default().labels(&selector)).await {
        Ok(l)  => l,
        Err(e) => { log_warn!("[k8s] pod list error: {}", e); return String::new(); }
    };
    let pod_name = match list.items.first().and_then(|p| p.metadata.name.clone()) {
        Some(n) => n,
        None    => { log_warn!("[k8s] no pod found for job '{}'", job_name); return String::new(); }
    };
    pods.logs(&pod_name, &LogParams::default()).await.unwrap_or_else(|e| {
        log_warn!("[k8s] log fetch error for '{}': {}", pod_name, e);
        String::new()
    })
}

pub async fn delete_job(client: &Client, namespace: &str, job_name: &str) {
    if let Err(e) = Api::<Job>::namespaced(client.clone(), namespace)
        .delete(job_name, &DeleteParams::foreground())
        .await
    {
        log_warn!("[k8s] failed to delete job '{}': {}", job_name, e);
    }
}
