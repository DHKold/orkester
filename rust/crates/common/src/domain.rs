//! Canonical domain model for Orkester objects loaded from the Workspace.
//!
//! Objects are described in YAML and share a common Kubernetes-like envelope:
//!
//! ```yaml
//! apiVersion: orkester.io/v1
//! kind: Namespace | Task | Work
//! name: my-object
//! version: "1.0.0"
//! metadata:
//!   description: "…"
//!   namespace: data-platform   # absent for Namespaces
//!   labels:
//!     team: data
//! spec:
//!   # kind-specific fields
//! ```

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

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

/// User-facing metadata attached to every object.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ObjectAnnotations {
    #[serde(default)]
    pub description: String,
    /// Namespace that owns this object.
    /// Empty / absent for Namespace objects themselves.
    #[serde(default)]
    pub namespace: String,
    #[serde(default)]
    pub labels: HashMap<String, String>,
}

// ── Namespace ─────────────────────────────────────────────────────────────────

/// A logical multi-tenancy boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Namespace {
    #[serde(flatten)]
    pub meta: ObjectMeta,
    #[serde(default)]
    pub spec: NamespaceSpec,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NamespaceSpec {
    /// Optional cap on the number of concurrently running Workflows.
    #[serde(default)]
    pub max_concurrent_workflows: Option<u32>,
}

// ── Task ─────────────────────────────────────────────────────────────────────

/// Definition of an atomic unit of work executed by a TaskExecutor.
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
    /// Executor-specific configuration, passed verbatim to the executor.
    #[serde(default)]
    pub config: serde_json::Value,
    /// Declared input parameters (name → human description).
    #[serde(default)]
    pub inputs: HashMap<String, String>,
    /// Declared output parameters (name → human description).
    #[serde(default)]
    pub outputs: HashMap<String, String>,
    /// Maximum wall-clock time before the task is considered failed (seconds).
    #[serde(default)]
    pub timeout_seconds: Option<u64>,
    /// Number of automatic retries on failure (0 = no retry).
    #[serde(default)]
    pub retries: u32,
}

// ── Work ─────────────────────────────────────────────────────────────────────

/// A DAG-oriented orchestration plan — defines how Tasks are wired together.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Work {
    #[serde(flatten)]
    pub meta: ObjectMeta,
    pub spec: WorkSpec,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkSpec {
    pub steps: Vec<WorkStep>,
    /// Input parameters required to instantiate a Workflow from this Work
    /// (name → human description).
    #[serde(default)]
    pub inputs: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkStep {
    /// Unique id for this step within the Work.
    pub id: String,
    /// Name of the Task to execute (resolved within the same namespace).
    pub task: String,
    /// Step ids that must complete successfully before this step can start.
    #[serde(default)]
    pub depends_on: Vec<String>,
    /// Static input overrides passed to the task at runtime.
    #[serde(default)]
    pub inputs: HashMap<String, String>,
    /// When `true`, a failure of this step does not abort the whole Work.
    #[serde(default)]
    pub allow_failure: bool,
}

// ── Discriminated envelope ────────────────────────────────────────────────────

/// Any object that may appear in a Workspace YAML document.
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
