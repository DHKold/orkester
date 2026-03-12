//! Domain model — the three object kinds managed by the Workspace server.
//!
//! All three share a common envelope:
//!
//! ```yaml
//! apiVersion: orkester.io/v1
//! kind: Namespace | Task | Work
//! name: my-object
//! version: "1.0.0"
//! metadata:
//!   description: "..."
//!   labels:
//!     team: data
//! spec:
//!   # kind-specific fields
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ── Common envelope ───────────────────────────────────────────────────────────

/// Common header present on every Orkester object.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectMeta {
    pub api_version: String,
    pub kind: String,
    pub name: String,
    /// Semantic version of this object definition (e.g. `"1.0.0"`).
    pub version: String,
    #[serde(default)]
    pub metadata: ObjectAnnotations,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ObjectAnnotations {
    #[serde(default)]
    pub description: String,
    /// Namespace that owns this object.  Empty ⇒ the default namespace.
    /// Namespaces themselves leave this field empty.
    #[serde(default)]
    pub namespace: String,
    #[serde(default)]
    pub labels: HashMap<String, String>,
}

// ── Namespace ─────────────────────────────────────────────────────────────────

/// A logical grouping / multi-tenancy boundary.
///
/// ```yaml
/// apiVersion: orkester.io/v1
/// kind: Namespace
/// name: acme
/// version: "1.0.0"
/// metadata:
///   description: "ACME tenant"
/// spec: {}
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Namespace {
    #[serde(flatten)]
    pub meta: ObjectMeta,
    #[serde(default)]
    pub spec: NamespaceSpec,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NamespaceSpec {
    /// Optional maximum number of concurrent WorkFlows in this namespace.
    #[serde(default)]
    pub max_concurrent_workflows: Option<u32>,
}

// ── Task ─────────────────────────────────────────────────────────────────────

/// Definition of an atomic unit of work.
///
/// The `spec.executor` field selects the executor plugin that will run this
/// task (e.g. `"eks-pod"`, `"command"`).  All executor-specific parameters
/// live inside `spec.config`.
///
/// ```yaml
/// apiVersion: orkester.io/v1
/// kind: Task
/// name: run-spark-job
/// version: "1.2.0"
/// metadata:
///   namespace: acme
///   description: "Run a Spark job in EKS"
///   labels:
///     team: data
/// spec:
///   executor: eks-pod
///   config:
///     image: apache/spark:3.5
///     command: ["spark-submit", "--class", "com.acme.Job"]
///     resources:
///       cpu: "4"
///       memory: "8Gi"
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    #[serde(flatten)]
    pub meta: ObjectMeta,
    pub spec: TaskSpec,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskSpec {
    /// Executor plugin id (e.g. `"eks-pod"`, `"command"`).
    pub executor: String,
    /// Executor-specific configuration passed verbatim to the executor.
    #[serde(default)]
    pub config: serde_json::Value,
    /// Input parameters expected by this task (name → description).
    #[serde(default)]
    pub inputs: HashMap<String, String>,
    /// Output parameters produced by this task (name → description).
    #[serde(default)]
    pub outputs: HashMap<String, String>,
    /// Maximum wall-clock time before the task is considered failed, in seconds.
    #[serde(default)]
    pub timeout_seconds: Option<u64>,
    /// Number of times to retry on failure (0 = no retry).
    #[serde(default)]
    pub retries: u32,
}

// ── Work ─────────────────────────────────────────────────────────────────────

/// A DAG-oriented orchestration plan: defines how tasks are wired together.
///
/// ```yaml
/// apiVersion: orkester.io/v1
/// kind: Work
/// name: nightly-pipeline
/// version: "2.0.0"
/// metadata:
///   namespace: acme
///   description: "Nightly data pipeline"
/// spec:
///   steps:
///     - id: extract
///       task: run-spark-job
///       inputs:
///         mode: "extract"
///     - id: transform
///       task: run-spark-job
///       dependsOn: [extract]
///       inputs:
///         mode: "transform"
///     - id: load
///       task: run-spark-job
///       dependsOn: [transform]
///       inputs:
///         mode: "load"
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Work {
    #[serde(flatten)]
    pub meta: ObjectMeta,
    pub spec: WorkSpec,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkSpec {
    pub steps: Vec<WorkStep>,
    /// Input parameters required to start a WorkFlow from this Work.
    #[serde(default)]
    pub inputs: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkStep {
    /// Unique id for this step within the Work.
    pub id: String,
    /// Name of the Task to execute (scoped to the same namespace as the Work).
    pub task: String,
    /// Step ids that must complete before this step can begin.
    #[serde(default)]
    pub depends_on: Vec<String>,
    /// Static input overrides passed to the task at runtime.
    #[serde(default)]
    pub inputs: HashMap<String, String>,
    /// If true, a failure of this step does not abort the whole Work.
    #[serde(default)]
    pub allow_failure: bool,
}

// ── Discriminated envelope ────────────────────────────────────────────────────

/// Parsed representation of any object that may appear in a YAML document.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum ObjectEnvelope {
    Namespace(Namespace),
    Task(Task),
    Work(Work),
}

impl ObjectEnvelope {
    pub fn kind(&self) -> &'static str {
        match self {
            ObjectEnvelope::Namespace(_) => "Namespace",
            ObjectEnvelope::Task(_) => "Task",
            ObjectEnvelope::Work(_) => "Work",
        }
    }

    pub fn name(&self) -> &str {
        match self {
            ObjectEnvelope::Namespace(n) => &n.meta.name,
            ObjectEnvelope::Task(t) => &t.meta.name,
            ObjectEnvelope::Work(w) => &w.meta.name,
        }
    }

    pub fn namespace(&self) -> &str {
        match self {
            ObjectEnvelope::Namespace(_) => "",
            ObjectEnvelope::Task(t) => &t.meta.metadata.namespace,
            ObjectEnvelope::Work(w) => &w.meta.metadata.namespace,
        }
    }
}
