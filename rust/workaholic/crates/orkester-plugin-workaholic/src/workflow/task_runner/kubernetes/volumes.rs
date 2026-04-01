//! Helpers for building Kubernetes resource requirements, environment injection,
//! and volume mount specs from `KubeJobConfig`.

use std::collections::BTreeMap;

use k8s_openapi::api::core::v1::{
    ConfigMapEnvSource, ConfigMapVolumeSource, EnvFromSource, ResourceRequirements,
    SecretEnvSource, SecretVolumeSource, Volume, VolumeMount,
};
use k8s_openapi::apimachinery::pkg::api::resource::Quantity;

use super::config::{KubeConfigMapEnvRef, KubeJobConfig, KubeSecretEnvRef, KubeVolumeMount};

/// Build a `ResourceRequirements` block from optional CPU/memory config.
///
/// Returns `None` when no resource constraints are configured.
pub fn build_resource_requirements(cfg: &KubeJobConfig) -> Option<ResourceRequirements> {
    let has_requests = cfg.cpu_request.is_some() || cfg.memory_request.is_some();
    let has_limits   = cfg.cpu_limit.is_some()   || cfg.memory_limit.is_some();
    if !has_requests && !has_limits {
        return None;
    }
    let requests = build_quantity_map(&cfg.cpu_request, &cfg.memory_request);
    let limits   = build_quantity_map(&cfg.cpu_limit,   &cfg.memory_limit);
    Some(ResourceRequirements {
        requests: if requests.is_empty() { None } else { Some(requests) },
        limits:   if limits.is_empty()   { None } else { Some(limits) },
        claims:   None,
    })
}

fn build_quantity_map(cpu: &Option<String>, memory: &Option<String>) -> BTreeMap<String, Quantity> {
    let mut map = BTreeMap::new();
    if let Some(v) = cpu    { map.insert("cpu".to_string(),    Quantity(v.clone())); }
    if let Some(v) = memory { map.insert("memory".to_string(), Quantity(v.clone())); }
    map
}

/// Build the `envFrom` list for the container from secret and configmap env references.
pub fn build_env_from(cfg: &KubeJobConfig) -> Vec<EnvFromSource> {
    let mut sources = Vec::new();
    for r in &cfg.secret_envs     { sources.push(env_from_secret(r)); }
    for r in &cfg.config_map_envs { sources.push(env_from_config_map(r)); }
    sources
}

fn env_from_secret(r: &KubeSecretEnvRef) -> EnvFromSource {
    EnvFromSource {
        prefix:         r.prefix.clone(),
        secret_ref:     Some(SecretEnvSource { name: r.secret.clone(), optional: Some(false) }),
        config_map_ref: None,
    }
}

fn env_from_config_map(r: &KubeConfigMapEnvRef) -> EnvFromSource {
    EnvFromSource {
        prefix:         r.prefix.clone(),
        config_map_ref: Some(ConfigMapEnvSource { name: r.config_map.clone(), optional: Some(false) }),
        secret_ref:     None,
    }
}

/// Build the `volumes` list for the PodSpec from the configured volume mounts.
pub fn build_pod_volumes(mounts: &[KubeVolumeMount]) -> Vec<Volume> {
    mounts.iter().map(pod_volume_from_cfg).collect()
}

fn pod_volume_from_cfg(m: &KubeVolumeMount) -> Volume {
    let secret = m.secret.as_deref().map(|name| SecretVolumeSource {
        secret_name: Some(name.to_string()),
        ..Default::default()
    });
    let config_map = m.config_map.as_deref().map(|name| ConfigMapVolumeSource {
        name: name.to_string(),
        ..Default::default()
    });
    Volume { name: m.name.clone(), secret, config_map, ..Default::default() }
}

/// Build the `volumeMounts` list for the Container from the configured volume mounts.
pub fn build_container_volume_mounts(mounts: &[KubeVolumeMount]) -> Vec<VolumeMount> {
    mounts.iter().map(container_volume_mount_from_cfg).collect()
}

fn container_volume_mount_from_cfg(m: &KubeVolumeMount) -> VolumeMount {
    VolumeMount {
        name:       m.name.clone(),
        mount_path: m.mount_path.clone(),
        read_only:  Some(m.read_only),
        ..Default::default()
    }
}
