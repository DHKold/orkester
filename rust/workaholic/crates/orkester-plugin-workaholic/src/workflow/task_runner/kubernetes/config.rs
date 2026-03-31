use serde_json::Value;
use workaholic::{TaskInputSource, TaskRunRequestDoc};

/// All configuration extracted from a task run request for a Kubernetes Job.
#[derive(Debug, Clone)]
pub struct KubeJobConfig {
    pub image:           String,
    pub namespace:       String,
    pub poll_secs:       u64,
    pub timeout_secs:    u64,
    pub service_account: Option<String>,
    pub command:         Vec<String>,
    pub args:            Vec<String>,
    pub env_vars:        Vec<(String, String)>,
}

/// Parse runner config keys out of the execution config block.
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
    })
}

fn as_string_vec(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(|v| v.as_array())
        .map(|a| a.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
        .unwrap_or_default()
}

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
