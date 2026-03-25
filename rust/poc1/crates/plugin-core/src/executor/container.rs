use std::collections::HashMap;

use async_trait::async_trait;
use orkester_common::plugin::providers::executor::{
    ExecutionRequest, ExecutionResult, ExecutionStatus, ExecutorBuilder, ExecutorError,
    TaskExecutor,
};
use serde_json::{json, Value};

/// Runs a task inside an OCI container using **Podman** or **Docker**.
///
/// The runtime is auto-detected (`podman` is tried first; falls back to
/// `docker`).  Override by setting `runtime` in the executor-level config
/// passed to [`ContainerExecutorBuilder::build`].
///
/// # Task config (`TaskSpec.config`)
///
/// ```yaml
/// spec:
///   executor: container
///   config:
///     image: "alpine:3.19"              # required
///
///     # Runtime selection (default: auto-detect podman → docker)
///     runtime: podman                   # podman | docker
///
///     # Image pull
///     pull_policy: if-not-present       # always | never | if-not-present (default)
///
///     # Process
///     entrypoint: "/bin/sh"             # override container ENTRYPOINT
///     command:                          # override container CMD
///       - "-c"
///       - "echo hello"
///     working_dir: "/work"              # -w / --workdir inside container
///     user: "1000:1000"                 # --user
///
///     # Environment
///     env:                              # static extra env vars
///       MY_VAR: "my-value"
///     env_files:                        # --env-file paths on the host
///       - /run/secrets/my.env
///
///     # Storage
///     volumes:                          # -v  host:container[:options]
///       - /data:/data:ro
///       - /tmp/work:/work
///     tmpfs:                            # --tmpfs  container-path[:options]
///       - /tmp
///
///     # Secrets & config files (Podman-only; silently ignored on Docker)
///     secrets:                          # --secret  name[,opt=val]
///       - my-db-password
///       - tls-cert,type=mount,target=/run/secrets/tls.crt
///
///     # Networking
///     network: host                     # --network
///     hostname: my-container            # --hostname
///     add_hosts:                        # --add-host  hostname:ip
///       - "db-host:10.0.0.5"
///
///     # Resource limits
///     memory: "512m"                    # --memory
///     cpus: "1.5"                       # --cpus
///
///     # Misc
///     privileged: false                 # --privileged
///     read_only: false                  # --read-only
///     extra_args:                       # appended verbatim before the image name
///       - "--security-opt=no-new-privileges"
/// ```
///
/// # Inputs → environment
///
/// Every input key is upper-cased and `.`/`-` replaced with `_`, then passed
/// as `-e KEY=value` to the container.
///
/// # Outputs
///
/// Declared outputs are captured via a small sentinel script appended to the
/// container command.  The executor wraps everything in `sh -c` and appends:
///
/// ```sh
/// echo '___ORKESTER_OUTPUTS___'
/// printf '%s=%s\n' 'OUT_VAR' "$OUT_VAR"
/// ```
///
/// | Key        | Type   | Description                                          |
/// |------------|--------|------------------------------------------------------|
/// | `$?`       | number | Exit code of the container process.                  |
/// | `<OUTPUT>` | string | Each name in `ExecutionRequest.outputs`, read from   |
/// |            |        | the container environment at exit.                   |
pub struct ContainerTaskExecutor {
    /// Explicit runtime path (e.g. `"podman"` or `"docker"`); `None` = auto-detect at execution time.
    runtime: Option<String>,
}

const OUTPUTS_SENTINEL: &str = "___ORKESTER_OUTPUTS___";

// ── Config parsing ────────────────────────────────────────────────────────────

struct ContainerConfig {
    image:        String,
    pull_policy:  PullPolicy,
    entrypoint:   Option<String>,
    command:      Vec<String>,
    working_dir:  Option<String>,
    user:         Option<String>,
    network:      Option<String>,
    hostname:     Option<String>,
    memory:       Option<String>,
    cpus:         Option<String>,
    privileged:   bool,
    read_only:    bool,
    env:          Vec<(String, String)>,
    env_files:    Vec<String>,
    volumes:      Vec<String>,
    tmpfs:        Vec<String>,
    secrets:      Vec<String>,
    add_hosts:    Vec<String>,
    extra_args:   Vec<String>,
}

#[derive(Debug)]
enum PullPolicy { Always, Never, IfNotPresent }

impl PullPolicy {
    fn from_str(s: &str) -> Self {
        match s {
            "always"         => Self::Always,
            "never"          => Self::Never,
            _                => Self::IfNotPresent,
        }
    }
}

impl ContainerConfig {
    fn parse(cfg: &Value, inputs: &HashMap<String, Value>) -> Result<Self, ExecutorError> {
        let image = cfg
            .get("image")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ExecutorError::Configuration("'image' is required".to_string()))?
            .to_string();

        let pull_policy = PullPolicy::from_str(
            cfg.get("pull_policy").and_then(|v| v.as_str()).unwrap_or("if-not-present"),
        );

        let entrypoint = cfg.get("entrypoint").and_then(|v| v.as_str()).map(str::to_owned);

        let command = cfg
            .get("command")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default();

        let working_dir = cfg.get("working_dir").and_then(|v| v.as_str()).map(str::to_owned);
        let user        = cfg.get("user").and_then(|v| v.as_str()).map(str::to_owned);
        let network     = cfg.get("network").and_then(|v| v.as_str()).map(str::to_owned);
        let hostname    = cfg.get("hostname").and_then(|v| v.as_str()).map(str::to_owned);
        let memory      = cfg.get("memory").and_then(|v| v.as_str()).map(str::to_owned);
        let cpus        = cfg.get("cpus").and_then(|v| v.as_str()).map(str::to_owned);
        let privileged  = cfg.get("privileged").and_then(|v| v.as_bool()).unwrap_or(false);
        let read_only   = cfg.get("read_only").and_then(|v| v.as_bool()).unwrap_or(false);

        // env: inputs (uppercased) first, then static config.env overrides
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
        if let Some(env_obj) = cfg.get("env").and_then(|v| v.as_object()) {
            for (k, v) in env_obj {
                if let Some(s) = v.as_str() {
                    env.push((k.clone(), s.to_owned()));
                }
            }
        }

        let env_files = str_array(cfg, "env_files");
        let volumes   = str_array(cfg, "volumes");
        let tmpfs     = str_array(cfg, "tmpfs");
        let secrets   = str_array(cfg, "secrets");
        let add_hosts = str_array(cfg, "add_hosts");
        let extra_args= str_array(cfg, "extra_args");

        Ok(ContainerConfig {
            image, pull_policy, entrypoint, command, working_dir, user,
            network, hostname, memory, cpus, privileged, read_only,
            env, env_files, volumes, tmpfs, secrets, add_hosts, extra_args,
        })
    }
}

fn str_array(cfg: &Value, key: &str) -> Vec<String> {
    cfg.get(key)
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(str::to_owned)).collect())
        .unwrap_or_default()
}

// ── TaskExecutor impl ─────────────────────────────────────────────────────────

#[async_trait]
impl TaskExecutor for ContainerTaskExecutor {
    async fn execute(&self, request: ExecutionRequest) -> Result<ExecutionResult, ExecutorError> {
        let cfg = ContainerConfig::parse(&request.task_definition, &request.inputs)?;
        let runtime = self.resolve_runtime()?;

        tracing::debug!(
            execution_id = %request.id,
            runtime      = %runtime,
            image        = %cfg.image,
            pull_policy  = ?cfg.pull_policy,
            "ContainerTaskExecutor: starting"
        );

        // ── Pull ──────────────────────────────────────────────────────────
        self.pull_if_needed(&runtime, &cfg.image, &cfg.pull_policy).await?;

        // ── Run ───────────────────────────────────────────────────────────
        let output = self
            .run_container(&runtime, &request.id, &cfg, &request.outputs)
            .await?;

        let stdout    = String::from_utf8_lossy(&output.stdout).into_owned();
        let stderr    = String::from_utf8_lossy(&output.stderr).into_owned();
        let exit_code = output.status.code().unwrap_or(-1);

        tracing::debug!(
            execution_id = %request.id,
            exit_code,
            stdout = stdout.trim(),
            stderr = stderr.trim(),
            "container finished"
        );

        let (outputs, log_stdout) = parse_outputs(&stdout, &request.outputs, exit_code);

        let mut logs: Vec<String> = log_stdout
            .lines()
            .map(|l| format!("[stdout] {l}"))
            .chain(stderr.lines().map(|l| format!("[stderr] {l}")))
            .collect();

        if logs.is_empty() {
            logs.push(format!("[container] exited with code {exit_code}"));
        }
        for line in &logs {
            orkester_common::log_info!("{}", line);
        }

        let status = if output.status.success() {
            ExecutionStatus::Succeeded
        } else {
            let msg = if stderr.trim().is_empty() {
                format!("container exited with code {exit_code}")
            } else {
                format!("container exited with code {exit_code}: {}", stderr.trim())
            };
            tracing::warn!(execution_id = %request.id, %msg, "container failed");
            ExecutionStatus::Failed(msg)
        };

        Ok(ExecutionResult { status, outputs, logs })
    }

    async fn cancel(&self, execution_id: &str) -> Result<(), ExecutorError> {
        // Container name is set to execution_id during run, so we can stop it.
        let container_name = container_name(execution_id);
        tracing::debug!(execution_id, %container_name, "cancelling container");
        let Ok(runtime) = self.resolve_runtime() else { return Ok(()); };
        let _ = tokio::process::Command::new(&runtime)
            .args(["stop", &container_name])
            .output()
            .await;
        Ok(())
    }
}

impl ContainerTaskExecutor {
    fn resolve_runtime(&self) -> Result<String, ExecutorError> {
        match &self.runtime {
            Some(r) => Ok(r.clone()),
            None => detect_runtime(),
        }
    }

    // ── Pull ──────────────────────────────────────────────────────────────

    async fn pull_if_needed(
        &self,
        runtime: &str,
        image: &str,
        policy: &PullPolicy,
    ) -> Result<(), ExecutorError> {
        match policy {
            PullPolicy::Never => return Ok(()),
            PullPolicy::IfNotPresent => {
                // Check whether the image is already present locally.
                let exists = tokio::process::Command::new(runtime)
                    .args(["image", "exists", image])
                    .status()
                    .await
                    .map(|s| s.success())
                    .unwrap_or(false);
                if exists {
                    tracing::debug!(image, "image already present — skipping pull");
                    return Ok(());
                }
            }
            PullPolicy::Always => {}
        }

        tracing::debug!(image, "pulling image");
        let out = tokio::process::Command::new(runtime)
            .args(["pull", image])
            .output()
            .await
            .map_err(|e| ExecutorError::Failed(format!("failed to pull image '{image}': {e}")))?;

        if !out.status.success() {
            let stderr = String::from_utf8_lossy(&out.stderr);
            return Err(ExecutorError::Failed(format!(
                "pull '{image}' failed: {stderr}"
            )));
        }
        Ok(())
    }

    // ── Run ───────────────────────────────────────────────────────────────

    async fn run_container(
        &self,
        runtime: &str,
        execution_id: &str,
        cfg: &ContainerConfig,
        declared_outputs: &[String],
    ) -> Result<std::process::Output, ExecutorError> {
        let mut args: Vec<String> = vec!["run".into(), "--rm".into()];

        // Stable, cancellable container name
        args.push("--name".into());
        args.push(container_name(execution_id));

        // Working directory
        if let Some(dir) = &cfg.working_dir {
            args.push("--workdir".into());
            args.push(dir.clone());
        }

        // User
        if let Some(u) = &cfg.user {
            args.push("--user".into());
            args.push(u.clone());
        }

        // Network
        if let Some(net) = &cfg.network {
            args.push("--network".into());
            args.push(net.clone());
        }

        // Hostname
        if let Some(h) = &cfg.hostname {
            args.push("--hostname".into());
            args.push(h.clone());
        }

        // Resource limits
        if let Some(mem) = &cfg.memory {
            args.push("--memory".into());
            args.push(mem.clone());
        }
        if let Some(c) = &cfg.cpus {
            args.push("--cpus".into());
            args.push(c.clone());
        }

        // Flags
        if cfg.privileged { args.push("--privileged".into()); }
        if cfg.read_only  { args.push("--read-only".into()); }

        // Volumes
        for v in &cfg.volumes {
            args.push("-v".into());
            args.push(v.clone());
        }

        // Tmpfs
        for t in &cfg.tmpfs {
            args.push("--tmpfs".into());
            args.push(t.clone());
        }

        // Env files
        for f in &cfg.env_files {
            args.push("--env-file".into());
            args.push(f.clone());
        }

        // Environment variables (inputs + static config.env)
        for (k, v) in &cfg.env {
            args.push("-e".into());
            args.push(format!("{k}={v}"));
        }

        // Secrets (Podman-only; Docker ignores unknown flags only if using
        // `--secret` via SwarmKit — we silently skip on Docker).
        if runtime.contains("podman") {
            for s in &cfg.secrets {
                args.push("--secret".into());
                args.push(s.clone());
            }
        }

        // Extra hosts
        for h in &cfg.add_hosts {
            args.push("--add-host".into());
            args.push(h.clone());
        }

        // Extra verbatim args (before image)
        args.extend(cfg.extra_args.iter().cloned());

        // Entrypoint override
        if let Some(ep) = &cfg.entrypoint {
            args.push("--entrypoint".into());
            args.push(ep.clone());
        }

        // Image
        args.push(cfg.image.clone());

        // Command / outputs injection
        //
        // If outputs are declared we wrap the user command in a shell so we can
        // append the sentinel output section.  Otherwise we pass the command
        // (if any) directly to avoid introducing a shell dependency.
        if !declared_outputs.is_empty() {
            // Each element of `command` is treated as a shell line/statement,
            // not as an individual word.  Join with newlines so multi-element
            // arrays run as sequential statements.
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

            if cfg.entrypoint.is_none() {
                // No entrypoint override — prefix with `sh -c` so the container
                // runs the script inline rather than looking for a file.
                args.push("sh".into());
                args.push("-c".into());
                args.push(script);
            } else {
                // Entrypoint is already a shell; pass the script via -c so it
                // is treated as inline code, not a filename.
                args.push("-c".into());
                args.push(script);
            }
        } else {
            // No outputs — pass command directly.
            args.extend(cfg.command.iter().cloned());
        }

        tracing::debug!(
            execution_id,
            runtime = %runtime,
            ?args,
            "spawning container"
        );

        tokio::process::Command::new(runtime)
            .args(&args)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .output()
            .await
            .map_err(|e| ExecutorError::Failed(format!("failed to spawn container: {e}")))
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Derive a deterministic, DNS-safe container name from the execution ID.
fn container_name(execution_id: &str) -> String {
    format!("orkester-{}", execution_id.replace(':', "-"))
}

/// Parse stdout for the sentinel line; return `(outputs_map, log_portion)`.
fn parse_outputs(
    stdout: &str,
    declared_outputs: &[String],
    exit_code: i32,
) -> (HashMap<String, Value>, String) {
    let mut outputs: HashMap<String, Value> = HashMap::new();
    outputs.insert("$?".to_string(), json!(exit_code));

    let lines: Vec<&str> = stdout.lines().collect();
    let sentinel_pos = lines.iter().position(|l| *l == OUTPUTS_SENTINEL);

    let log_stdout = match sentinel_pos {
        None => stdout.to_string(),
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

    (outputs, log_stdout)
}

// ── Builder ───────────────────────────────────────────────────────────────────

pub struct ContainerExecutorBuilder;

impl ExecutorBuilder for ContainerExecutorBuilder {
    fn build(&self, config: Value) -> Result<Box<dyn TaskExecutor>, ExecutorError> {
        // Allow explicit runtime override in the executor-level config;
        // otherwise defer auto-detection to execute time.
        let runtime = config
            .get("runtime")
            .and_then(|v| v.as_str())
            .map(str::to_string);
        Ok(Box::new(ContainerTaskExecutor { runtime }))
    }
}

/// Try `podman` first, then `docker`.  Returns an error if neither is found.
fn detect_runtime() -> Result<String, ExecutorError> {
    for candidate in &["podman", "docker"] {
        if std::process::Command::new(candidate)
            .arg("--version")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
        {
            return Ok(candidate.to_string());
        }
    }
    Err(ExecutorError::Configuration(
        "no container runtime found — install podman or docker".to_string(),
    ))
}

