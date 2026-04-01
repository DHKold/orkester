use std::collections::BTreeMap;
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
use super::volumes::{build_container_volume_mounts, build_env_from, build_pod_volumes, build_resource_requirements};

/// Build the container spec including env vars, envFrom, resource limits, and volume mounts.
fn build_container(cfg: &KubeJobConfig, env: Vec<EnvVar>) -> Container {
    let env_from      = build_env_from(cfg);
    let volume_mounts = build_container_volume_mounts(&cfg.volume_mounts);
    Container {
        name:          "task".to_string(),
        image:         Some(cfg.image.clone()),
        command:       if cfg.command.is_empty()      { None } else { Some(cfg.command.clone()) },
        args:          if cfg.args.is_empty()          { None } else { Some(cfg.args.clone()) },
        env:           if env.is_empty()               { None } else { Some(env) },
        env_from:      if env_from.is_empty()          { None } else { Some(env_from) },
        resources:     build_resource_requirements(cfg),
        volume_mounts: if volume_mounts.is_empty()     { None } else { Some(volume_mounts) },
        ..Default::default()
    }
}

/// Build Kubernetes annotations from task run metadata for observability.
fn build_annotations(cfg: &KubeJobConfig) -> BTreeMap<String, String> {
    BTreeMap::from([
        ("workaholic/work-ref".to_string(), cfg.work_ref.clone()),
        ("workaholic/task-ref".to_string(), cfg.task_ref.clone()),
    ])
}

/// Build the complete Kubernetes Job manifest.
fn build_job(job_name: &str, cfg: &KubeJobConfig, container: Container) -> Job {
    let volumes = build_pod_volumes(&cfg.volume_mounts);
    Job {
        metadata: ObjectMeta {
            name:        Some(job_name.to_string()),
            namespace:   Some(cfg.namespace.clone()),
            annotations: Some(build_annotations(cfg)),
            ..Default::default()
        },
        spec: Some(JobSpec {
            backoff_limit: Some(0),
            template: PodTemplateSpec {
                spec: Some(PodSpec {
                    restart_policy:       Some("Never".to_string()),
                    service_account_name: cfg.service_account.clone(),
                    containers:           vec![container],
                    volumes:              if volumes.is_empty() { None } else { Some(volumes) },
                    ..Default::default()
                }),
                ..Default::default()
            },
            ..Default::default()
        }),
        ..Default::default()
    }
}

/// Submit the Kubernetes Job to the cluster.
pub async fn create_job(client: &Client, cfg: &KubeJobConfig, job_name: &str) -> Result<(), kube::Error> {
    let env = cfg.env_vars.iter()
        .map(|(k, v)| EnvVar { name: k.clone(), value: Some(v.clone()), ..Default::default() })
        .collect();
    let job = build_job(job_name, cfg, build_container(cfg, env));
    Api::<Job>::namespaced(client.clone(), &cfg.namespace)
        .create(&PostParams::default(), &job)
        .await?;
    Ok(())
}

/// Poll the Job status until it reaches a terminal state or the timeout is exceeded.
pub async fn wait_for_completion(
    client:   &Client,
    cfg:      &KubeJobConfig,
    job_name: &str,
    state:    &Arc<Mutex<KubeTaskRunState>>,
) -> TaskRunState {
    let jobs: Api<Job> = Api::namespaced(client.clone(), &cfg.namespace);
    let deadline       = Instant::now() + Duration::from_secs(cfg.timeout_secs);
    loop {
        if state.lock().unwrap().cancel_requested { return TaskRunState::Cancelled; }
        if Instant::now() >= deadline {
            log_warn!("[k8s] job '{}' timed out after {}s", job_name, cfg.timeout_secs);
            return TaskRunState::Failed;
        }
        match jobs.get(job_name).await {
            Ok(job) => {
                if let Some(s) = job.status {
                    if s.succeeded.unwrap_or(0) > 0 { return TaskRunState::Succeeded; }
                    if s.failed.unwrap_or(0)    > 0 { return TaskRunState::Failed; }
                }
            }
            Err(e) => log_warn!("[k8s] status query error for '{}': {}", job_name, e),
        }
        tokio::time::sleep(Duration::from_secs(cfg.poll_secs)).await;
    }
}

/// Fetch stdout logs from the first pod that ran the Job.
pub async fn fetch_logs(client: &Client, namespace: &str, job_name: &str) -> String {
    let pods: Api<Pod> = Api::namespaced(client.clone(), namespace);
    let selector = format!("job-name={job_name}");
    let list = match pods.list(&ListParams::default().labels(&selector)).await {
        Ok(l)  => l,
        Err(e) => { log_warn!("[k8s] pod list error for job '{}': {}", job_name, e); return String::new(); }
    };
    let pod_name = match list.items.first().and_then(|p| p.metadata.name.clone()) {
        Some(n) => n,
        None    => { log_warn!("[k8s] no pod found for job '{}'", job_name); return String::new(); }
    };
    pods.logs(&pod_name, &LogParams::default()).await.unwrap_or_else(|e| {
        log_warn!("[k8s] log fetch error for pod '{}': {}", pod_name, e);
        String::new()
    })
}

/// Delete the Kubernetes Job using foreground propagation to also clean up its pods.
pub async fn delete_job(client: &Client, namespace: &str, job_name: &str) {
    if let Err(e) = Api::<Job>::namespaced(client.clone(), namespace)
        .delete(job_name, &DeleteParams::foreground())
        .await
    {
        log_warn!("[k8s] failed to delete job '{}': {}", job_name, e);
    }
}

