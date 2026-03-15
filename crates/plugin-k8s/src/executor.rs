use std::collections::HashMap;
use std::time::Duration;

use async_trait::async_trait;
use k8s_openapi::api::core::v1::{
    ConfigMapVolumeSource, EnvVar, EnvVarSource, Pod, PodSpec, ResourceRequirements,
    SecretKeySelector, SecretVolumeSource, Volume, VolumeMount,
};
use k8s_openapi::apimachinery::pkg::api::resource::Quantity;
use kube::api::{Api, DeleteParams, PostParams};
use kube::runtime::wait::{await_condition, conditions};
use kube::Client;
use orkester_common::plugin::providers::executor::{
    ExecutionRequest, ExecutionResult, ExecutionStatus, ExecutorBuilder, ExecutorError,
    TaskExecutor,
};
use serde_json::{json, Value};

// ── Config ────────────────────────────────────────────────────────────────────

/// Configuration block read from `TaskSpec.config` for a `kubernetes` executor task.
///
/// ```yaml
/// spec:
///   executor: kubernetes
///   config:
///     image: "python:3.12-slim"          # required
///
///     # Namespace where the Pod is created (default: "default")
///     namespace: "my-workflows"
///
///     # Image pull policy: Always | Never | IfNotPresent (default)
///     pull_policy: IfNotPresent
///
///     # Service account to bind (optional)
///     service_account: "workflow-runner"
///
///     # Resource requests/limits
///     resources:
///       requests:
///         cpu: "100m"
///         memory: "128Mi"
///       limits:
///         cpu: "500m"
///         memory: "256Mi"
///
///     # Command to run (overrides container ENTRYPOINT + CMD)
///     command:
///       - "sh"
///       - "-c"
///       - "echo hello"
///
///     # Working directory inside the container
///     working_dir: "/work"
///
///     # Static extra environment variables
///     env:
///       MY_VAR: "my-value"
///
///     # Inject a Kubernetes Secret key as an env var
///     env_from_secret:
///       DB_PASSWORD:
///         secret: "db-creds"
///         key: "password"
///
///     # Volume mounts: name → { secret|configmap, mount_path }
///     volumes:
///       - name: tls-cert
///         secret: "tls-secret"
///         mount_path: "/run/secrets/tls"
///       - name: app-config
///         config_map: "app-config"
///         mount_path: "/etc/app"
///
///     # Timeout for the Pod to finish (seconds, default 300)
///     timeout_seconds: 120
///
///     # Node selector labels
///     node_selector:
///       kubernetes.io/arch: amd64
///
///     # Tolerations (raw list forwarded verbatim)
///     tolerations: []
/// ```
///
/// # Inputs → environment variables
///
/// Every input key is upper-cased (`-`/`.` → `_`) and injected as an env var.
///
/// # Outputs
///
/// Declared outputs are captured via the sentinel pattern: the executor wraps
/// the user command in `sh -c` and appends:
/// ```sh
/// echo '___ORKESTER_OUTPUTS___'
/// printf '%s=%s\n' 'VAR' "$VAR"
/// ```
/// Stdout is scanned after the sentinel; matched lines populate the step output map.
struct K8sConfig {
    namespace:       String,
    image:           String,
    pull_policy:     String,
    service_account: Option<String>,
    command:         Vec<String>,
    working_dir:     Option<String>,
    env:             Vec<(String, String)>,
    env_from_secret: Vec<(String, String, String)>, // (env_name, secret_name, key)
    volumes:         Vec<VolumeSpec>,
    resources:       Option<ResourceSpec>,
    timeout_seconds: u64,
    node_selector:   HashMap<String, String>,
}

#[derive(Debug)]
struct VolumeSpec {
    name:       String,
    source:     VolumeSource,
    mount_path: String,
}

#[derive(Debug)]
enum VolumeSource {
    Secret(String),
    ConfigMap(String),
}

#[derive(Debug)]
struct ResourceSpec {
    cpu_request:    Option<String>,
    memory_request: Option<String>,
    cpu_limit:      Option<String>,
    memory_limit:   Option<String>,
}

const OUTPUTS_SENTINEL: &str = "___ORKESTER_OUTPUTS___";

impl K8sConfig {
    fn parse(cfg: &Value, inputs: &HashMap<String, Value>) -> Result<Self, ExecutorError> {
        let image = cfg
            .get("image")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ExecutorError::Configuration("'image' is required".into()))?
            .to_string();

        let namespace = cfg
            .get("namespace")
            .and_then(|v| v.as_str())
            .unwrap_or("default")
            .to_string();

        let pull_policy = cfg
            .get("pull_policy")
            .and_then(|v| v.as_str())
            .unwrap_or("IfNotPresent")
            .to_string();

        let service_account = cfg
            .get("service_account")
            .and_then(|v| v.as_str())
            .map(str::to_owned);

        let command: Vec<String> = cfg
            .get("command")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(str::to_owned)).collect())
            .unwrap_or_default();

        let working_dir = cfg.get("working_dir").and_then(|v| v.as_str()).map(str::to_owned);

        // env: inputs first (uppercased), then static config.env overrides
        let mut env: Vec<(String, String)> = inputs
            .iter()
            .map(|(k, v)| {
                let key = k.to_uppercase().replace(['.', '-'], "_");
                let val = match v {
                    Value::String(s) => s.clone(),
                    other => other.to_string(),
                };
                (key, val)
            })
            .collect();
        if let Some(obj) = cfg.get("env").and_then(|v| v.as_object()) {
            for (k, v) in obj {
                if let Some(s) = v.as_str() {
                    env.push((k.clone(), s.to_owned()));
                }
            }
        }

        // env_from_secret: { ENV_KEY: { secret: "name", key: "key" } }
        let mut env_from_secret = Vec::new();
        if let Some(obj) = cfg.get("env_from_secret").and_then(|v| v.as_object()) {
            for (env_name, spec) in obj {
                let secret = spec.get("secret").and_then(|v| v.as_str()).ok_or_else(|| {
                    ExecutorError::Configuration(format!(
                        "env_from_secret.{env_name}: 'secret' is required"
                    ))
                })?;
                let key = spec.get("key").and_then(|v| v.as_str()).ok_or_else(|| {
                    ExecutorError::Configuration(format!(
                        "env_from_secret.{env_name}: 'key' is required"
                    ))
                })?;
                env_from_secret.push((env_name.clone(), secret.to_owned(), key.to_owned()));
            }
        }

        // volumes
        let mut volumes = Vec::new();
        if let Some(arr) = cfg.get("volumes").and_then(|v| v.as_array()) {
            for item in arr {
                let name = item
                    .get("name")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| ExecutorError::Configuration("volume entry missing 'name'".into()))?
                    .to_string();
                let mount_path = item
                    .get("mount_path")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        ExecutorError::Configuration(format!("volume '{name}' missing 'mount_path'"))
                    })?
                    .to_string();
                let source = if let Some(s) = item.get("secret").and_then(|v| v.as_str()) {
                    VolumeSource::Secret(s.to_owned())
                } else if let Some(c) = item.get("config_map").and_then(|v| v.as_str()) {
                    VolumeSource::ConfigMap(c.to_owned())
                } else {
                    return Err(ExecutorError::Configuration(format!(
                        "volume '{name}' must have either 'secret' or 'config_map'"
                    )));
                };
                volumes.push(VolumeSpec { name, source, mount_path });
            }
        }

        // resources
        let resources = cfg.get("resources").map(|r| {
            let req = r.get("requests");
            let lim = r.get("limits");
            ResourceSpec {
                cpu_request:    req.and_then(|v| v.get("cpu")).and_then(|v| v.as_str()).map(str::to_owned),
                memory_request: req.and_then(|v| v.get("memory")).and_then(|v| v.as_str()).map(str::to_owned),
                cpu_limit:      lim.and_then(|v| v.get("cpu")).and_then(|v| v.as_str()).map(str::to_owned),
                memory_limit:   lim.and_then(|v| v.get("memory")).and_then(|v| v.as_str()).map(str::to_owned),
            }
        });

        let timeout_seconds = cfg
            .get("timeout_seconds")
            .and_then(|v| v.as_u64())
            .unwrap_or(300);

        let node_selector: HashMap<String, String> = cfg
            .get("node_selector")
            .and_then(|v| v.as_object())
            .map(|obj| {
                obj.iter()
                    .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_owned())))
                    .collect()
            })
            .unwrap_or_default();

        Ok(K8sConfig {
            namespace,
            image,
            pull_policy,
            service_account,
            command,
            working_dir,
            env,
            env_from_secret,
            volumes,
            resources,
            timeout_seconds,
            node_selector,
        })
    }
}

// ── Executor ──────────────────────────────────────────────────────────────────

pub struct KubernetesTaskExecutor;

#[async_trait]
impl TaskExecutor for KubernetesTaskExecutor {
    async fn execute(&self, request: ExecutionRequest) -> Result<ExecutionResult, ExecutorError> {
        let cfg = K8sConfig::parse(&request.task_definition, &request.inputs)?;

        let client = Client::try_default().await.map_err(|e| {
            ExecutorError::Configuration(format!("cannot build Kubernetes client: {e}"))
        })?;

        let pod_name = pod_name(&request.id);
        let pods: Api<Pod> = Api::namespaced(client.clone(), &cfg.namespace);

        let pod = build_pod(&pod_name, &cfg, &request.outputs)?;

        tracing::debug!(
            execution_id = %request.id,
            %pod_name,
            namespace = %cfg.namespace,
            image = %cfg.image,
            "KubernetesTaskExecutor: creating pod"
        );

        pods.create(&PostParams::default(), &pod).await.map_err(|e| {
            ExecutorError::Failed(format!("failed to create pod '{pod_name}': {e}"))
        })?;

        // Wait for pod to reach terminal state
        let timeout = Duration::from_secs(cfg.timeout_seconds);
        let result = tokio::time::timeout(
            timeout,
            await_condition(pods.clone(), &pod_name, conditions::is_pod_running()),
        )
        .await;

        // Collect logs regardless of outcome
        let logs = fetch_logs(&pods, &pod_name).await;

        // Parse the pod phase after it finishes (re-read it)
        let phase = get_pod_phase(&pods, &pod_name).await;

        // Always clean up
        let _ = pods.delete(&pod_name, &DeleteParams::default()).await;

        match result {
            Err(_elapsed) => {
                return Err(ExecutorError::Failed(format!(
                    "pod '{pod_name}' timed out after {}s",
                    cfg.timeout_seconds
                )));
            }
            Ok(Err(e)) => {
                return Err(ExecutorError::Failed(format!(
                    "error waiting for pod '{pod_name}': {e}"
                )));
            }
            Ok(Ok(_)) => {}
        }

        let log_text = logs.join("\n");
        let exit_code: i32 = if phase == "Succeeded" { 0 } else { 1 };
        let (outputs, log_str) = parse_outputs(&log_text, &request.outputs, exit_code);

        let log_lines: Vec<String> = log_str.lines().map(str::to_owned).collect();

        let status = if phase == "Succeeded" {
            ExecutionStatus::Succeeded
        } else {
            let msg = format!("pod '{pod_name}' finished with phase '{phase}'");
            tracing::warn!(%msg, "kubernetes task failed");
            ExecutionStatus::Failed(msg)
        };

        Ok(ExecutionResult { status, outputs, logs: log_lines })
    }

    async fn cancel(&self, execution_id: &str) -> Result<(), ExecutorError> {
        let pod_name = pod_name(execution_id);
        // Best-effort: if we can't build the client or find the namespace we just return Ok.
        let Ok(client) = Client::try_default().await else { return Ok(()) };
        // We don't know the namespace here; default is the safest bet.
        let pods: Api<Pod> = Api::namespaced(client, "default");
        let _ = pods.delete(&pod_name, &DeleteParams::default()).await;
        Ok(())
    }
}

// ── Pod builder ───────────────────────────────────────────────────────────────

fn build_pod(
    pod_name: &str,
    cfg: &K8sConfig,
    declared_outputs: &[String],
) -> Result<Pod, ExecutorError> {
    use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;

    // Build the shell script that wraps the user command and appends the
    // sentinel output section.
    let (cmd_args, command_override) = build_command(cfg, declared_outputs);

    // Static env vars
    let mut env_vars: Vec<EnvVar> = cfg
        .env
        .iter()
        .map(|(k, v)| EnvVar {
            name:  k.clone(),
            value: Some(v.clone()),
            ..Default::default()
        })
        .collect();

    // Secret-sourced env vars
    for (env_name, secret_name, secret_key) in &cfg.env_from_secret {
        env_vars.push(EnvVar {
            name: env_name.clone(),
            value: None,
            value_from: Some(EnvVarSource {
                secret_key_ref: Some(SecretKeySelector {
                    name: secret_name.clone(),
                    key: secret_key.clone(),
                    optional: Some(false),
                }),
                ..Default::default()
            }),
        });
    }

    // Volumes + mounts
    let (volumes, volume_mounts) = build_volumes(cfg);

    // Resource requirements
    let resources = cfg.resources.as_ref().map(|r| {
        let mut requests = std::collections::BTreeMap::new();
        let mut limits   = std::collections::BTreeMap::new();
        if let Some(v) = &r.cpu_request    { requests.insert("cpu".into(), Quantity(v.clone())); }
        if let Some(v) = &r.memory_request { requests.insert("memory".into(), Quantity(v.clone())); }
        if let Some(v) = &r.cpu_limit      { limits.insert("cpu".into(), Quantity(v.clone())); }
        if let Some(v) = &r.memory_limit   { limits.insert("memory".into(), Quantity(v.clone())); }
        ResourceRequirements {
            requests: if requests.is_empty() { None } else { Some(requests) },
            limits:   if limits.is_empty()   { None } else { Some(limits) },
            ..Default::default()
        }
    });

    let container = k8s_openapi::api::core::v1::Container {
        name:             "task".to_string(),
        image:            Some(cfg.image.clone()),
        image_pull_policy: Some(cfg.pull_policy.clone()),
        command:          command_override,
        args:             if cmd_args.is_empty() { None } else { Some(cmd_args) },
        working_dir:      cfg.working_dir.clone(),
        env:              if env_vars.is_empty() { None } else { Some(env_vars) },
        volume_mounts:    if volume_mounts.is_empty() { None } else { Some(volume_mounts) },
        resources,
        ..Default::default()
    };

    let node_selector = if cfg.node_selector.is_empty() {
        None
    } else {
        Some(cfg.node_selector.clone().into_iter().collect())
    };

    let pod = Pod {
        metadata: ObjectMeta {
            name:       Some(pod_name.to_string()),
            namespace:  Some(cfg.namespace.clone()),
            labels:     Some([("app.kubernetes.io/managed-by".into(), "orkester".into())].into()),
            annotations: Some([("orkester/pod-name".into(), pod_name.to_string())].into()),
            ..Default::default()
        },
        spec: Some(PodSpec {
            restart_policy:     Some("Never".to_string()),
            service_account_name: cfg.service_account.clone(),
            containers:          vec![container],
            volumes:             if volumes.is_empty() { None } else { Some(volumes) },
            node_selector,
            ..Default::default()
        }),
        ..Default::default()
    };

    Ok(pod)
}

/// Returns `(args, command_override)` pair for the container spec.
///
/// When outputs are declared the user command is wrapped in `sh -c` with an
/// appended sentinel section.  Otherwise the command is passed through as-is.
fn build_command(
    cfg: &K8sConfig,
    declared_outputs: &[String],
) -> (Vec<String>, Option<Vec<String>>) {
    if declared_outputs.is_empty() {
        // Pass command / args through directly; no wrapping needed.
        if cfg.command.is_empty() {
            (vec![], None)
        } else {
            // First element is treated as the entrypoint, remainder as args.
            let mut it = cfg.command.iter().cloned();
            let cmd = it.next().unwrap();
            (it.collect(), Some(vec![cmd]))
        }
    } else {
        // Wrap everything in a sh -c script that appends the sentinel section.
        let inner_cmd = cfg.command.join("\n");

        let mut script_lines: Vec<String> = vec!["set -e".into()];
        if !inner_cmd.is_empty() {
            script_lines.push(inner_cmd);
        }
        script_lines.push("set +e".into());
        script_lines.push(format!("echo '{OUTPUTS_SENTINEL}'"));
        for name in declared_outputs {
            script_lines.push(format!("printf '%s=%s\\n' '{name}' \"${name}\""));
        }
        let script = script_lines.join("\n");

        (vec![script], Some(vec!["sh".into(), "-c".into()]))
    }
}

fn build_volumes(cfg: &K8sConfig) -> (Vec<Volume>, Vec<VolumeMount>) {
    let mut volumes      = Vec::new();
    let mut volume_mounts = Vec::new();

    for spec in &cfg.volumes {
        let vol = match &spec.source {
            VolumeSource::Secret(name) => Volume {
                name:   spec.name.clone(),
                secret: Some(SecretVolumeSource {
                    secret_name: Some(name.clone()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            VolumeSource::ConfigMap(name) => Volume {
                name:       spec.name.clone(),
                config_map: Some(ConfigMapVolumeSource {
                    name: name.clone(),
                    ..Default::default()
                }),
                ..Default::default()
            },
        };
        volumes.push(vol);

        volume_mounts.push(VolumeMount {
            name:       spec.name.clone(),
            mount_path: spec.mount_path.clone(),
            read_only:  Some(true),
            ..Default::default()
        });
    }

    (volumes, volume_mounts)
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// DNS-safe pod name derived from the execution ID.
fn pod_name(execution_id: &str) -> String {
    // UUIDs are already DNS-safe; just prefix.
    format!("orkester-{}", execution_id.replace('_', "-").to_lowercase())
}

async fn fetch_logs(pods: &Api<Pod>, pod_name: &str) -> Vec<String> {
    match pods.logs(pod_name, &Default::default()).await {
        Ok(text) => text.lines().map(str::to_owned).collect(),
        Err(e) => {
            tracing::warn!(%pod_name, error = %e, "could not fetch pod logs");
            vec![]
        }
    }
}

async fn get_pod_phase(pods: &Api<Pod>, pod_name: &str) -> String {
    match pods.get(pod_name).await {
        Ok(pod) => pod
            .status
            .and_then(|s| s.phase)
            .unwrap_or_else(|| "Unknown".to_string()),
        Err(_) => "Unknown".to_string(),
    }
}

fn parse_outputs(
    log_text: &str,
    declared_outputs: &[String],
    exit_code: i32,
) -> (HashMap<String, Value>, String) {
    let mut outputs: HashMap<String, Value> = HashMap::new();
    outputs.insert("$?".to_string(), json!(exit_code));

    let lines: Vec<&str> = log_text.lines().collect();
    let sentinel_pos = lines.iter().position(|l| *l == OUTPUTS_SENTINEL);

    let log_str = match sentinel_pos {
        None => log_text.to_string(),
        Some(pos) => {
            for line in &lines[pos + 1..] {
                if let Some((k, v)) = line.split_once('=') {
                    if declared_outputs.iter().any(|o| o == k) {
                        outputs.insert(k.to_string(), json!(v));
                    }
                }
            }
            lines[..pos].join("\n")
        }
    };

    (outputs, log_str)
}

// ── Builder ───────────────────────────────────────────────────────────────────

pub struct KubernetesExecutorBuilder;

impl ExecutorBuilder for KubernetesExecutorBuilder {
    fn build(
        &self,
        _config: Value,
    ) -> Result<Box<dyn TaskExecutor>, ExecutorError> {
        // The Kubernetes client is created lazily per execution using the
        // in-cluster service account or ~/.kube/config on the host.
        // No builder-level config is required.
        Ok(Box::new(KubernetesTaskExecutor))
    }
}
