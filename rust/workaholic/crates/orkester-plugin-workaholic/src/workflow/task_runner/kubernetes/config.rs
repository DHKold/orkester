use serde::Deserialize;
use serde_json::Value;
use workaholic::{TaskInputSource, TaskRunRequestDoc};

/// Injects all keys from a Kubernetes Secret as environment variables into the container.
#[derive(Debug, Clone, Deserialize)]
pub struct KubeSecretEnvRef {
    /// Name of the Kubernetes Secret.
    pub secret: String,
    /// Optional prefix to prepend to every injected environment variable name.
    #[serde(default)]
    pub prefix: Option<String>,
}

/// Injects all keys from a Kubernetes ConfigMap as environment variables into the container.
#[derive(Debug, Clone, Deserialize)]
pub struct KubeConfigMapEnvRef {
    /// Name of the Kubernetes ConfigMap.
    pub config_map: String,
    /// Optional prefix to prepend to every injected environment variable name.
    #[serde(default)]
    pub prefix: Option<String>,
}

/// Mounts a Kubernetes Secret or ConfigMap as a read-only directory of files.
#[derive(Debug, Clone, Deserialize)]
pub struct KubeVolumeMount {
    /// Volume name — must be unique within the pod spec.
    pub name: String,
    /// Name of the Secret to mount (mutually exclusive with `config_map`).
    pub secret: Option<String>,
    /// Name of the ConfigMap to mount (mutually exclusive with `secret`).
    pub config_map: Option<String>,
    /// Absolute path inside the container where the volume will appear.
    pub mount_path: String,
    /// Whether to mount the volume as read-only. Defaults to `true`.
    #[serde(default = "default_read_only")]
    pub read_only: bool,
}

fn default_read_only() -> bool { true }

/// All configuration extracted from a task run request for a Kubernetes Job.
#[derive(Debug, Clone)]
pub struct KubeJobConfig {
    pub image:              String,
    pub namespace:          String,
    pub poll_secs:          u64,
    pub timeout_secs:       u64,
    pub service_account:    Option<String>,
    pub command:            Vec<String>,
    pub args:               Vec<String>,
    pub env_vars:           Vec<(String, String)>,
    /// Prefix used when naming the Kubernetes Job. Default: `"wh-run-"`.
    pub job_name_prefix:    String,
    // Observability — stored as Job annotations.
    pub work_ref:           String,
    pub task_ref:           String,
    // Resource constraints.
    pub cpu_request:        Option<String>,
    pub cpu_limit:          Option<String>,
    pub memory_request:     Option<String>,
    pub memory_limit:       Option<String>,
    // Secret / ConfigMap env injection and file mounts.
    pub secret_envs:        Vec<KubeSecretEnvRef>,
    pub config_map_envs:    Vec<KubeConfigMapEnvRef>,
    pub volume_mounts:      Vec<KubeVolumeMount>,
}

/// Parse all runner config keys from the task run request's execution config block.
pub fn parse_config(request: &TaskRunRequestDoc) -> Result<KubeJobConfig, String> {
    let cfg = &request.spec.execution.config;
    let image = cfg
        .get("image")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "missing 'image' in kubernetes runner config".to_string())?
        .to_string();
    Ok(KubeJobConfig {
        image,
        namespace:       cfg.get("namespace").and_then(|v| v.as_str()).unwrap_or("default").to_string(),
        poll_secs:       cfg.get("poll_secs").and_then(|v| v.as_u64()).unwrap_or(5),
        timeout_secs:    cfg.get("timeout_secs").and_then(|v| v.as_u64()).unwrap_or(3600),
        service_account: cfg.get("service_account").and_then(|v| v.as_str()).map(|s| s.to_string()),
        command:         as_string_vec(cfg.get("command")),
        args:            as_string_vec(cfg.get("args")),
        env_vars:        collect_env_vars(request),
        job_name_prefix: cfg.get("job_name_prefix").and_then(|v| v.as_str()).unwrap_or("wh-run-").to_string(),
        work_ref:        request.spec.work_ref.clone(),
        task_ref:        request.spec.task_ref.clone(),
        cpu_request:     cfg.get("cpu_request").and_then(|v| v.as_str()).map(|s| s.to_string()),
        cpu_limit:       cfg.get("cpu_limit").and_then(|v| v.as_str()).map(|s| s.to_string()),
        memory_request:  cfg.get("memory_request").and_then(|v| v.as_str()).map(|s| s.to_string()),
        memory_limit:    cfg.get("memory_limit").and_then(|v| v.as_str()).map(|s| s.to_string()),
        secret_envs:     parse_typed_array(cfg.get("secret_envs"))?,
        config_map_envs: parse_typed_array(cfg.get("config_map_envs"))?,
        volume_mounts:   parse_typed_array(cfg.get("volume_mounts"))?,
    })
}

/// Deserialize an optional JSON array into a typed Vec, returning an error on malformed input.
fn parse_typed_array<T: for<'de> Deserialize<'de>>(value: Option<&Value>) -> Result<Vec<T>, String> {
    match value {
        None    => Ok(Vec::new()),
        Some(v) => serde_json::from_value(v.clone()).map_err(|e| e.to_string()),
    }
}

fn as_string_vec(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(|v| v.as_array())
        .map(|a| a.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
        .unwrap_or_default()
}

/// Collect resolved task inputs as `(name, value)` pairs for injection as environment variables.
pub fn collect_env_vars(request: &TaskRunRequestDoc) -> Vec<(String, String)> {
    request.spec.inputs.iter()
        .map(|i| {
            let v = match &i.from {
                TaskInputSource::Literal { value } =>
                    value.as_str().map(|s| s.to_string()).unwrap_or_else(|| value.to_string()),
                TaskInputSource::ArtifactRef { uri } => uri.clone(),
            };
            (i.name.clone(), v)
        })
        .collect()
}
