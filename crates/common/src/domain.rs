use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ── Identifiers ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct WorkspaceId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct WorkId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TaskId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ArtifactId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ExecutionId(pub String);

// ── Workspace ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workspace {
    pub id: WorkspaceId,
    pub name: String,
    pub description: Option<String>,
    pub metadata: HashMap<String, String>,
}

// ── Artifact ──────────────────────────────────────────────────────────────────

/// The kind of an artifact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactKind {
    /// Plain data/file artifact.
    Data,
    /// A secret (credentials, API key, etc.) — handled with extra care.
    Secret,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artifact {
    pub id: ArtifactId,
    pub kind: ArtifactKind,
    pub name: String,
    /// Opaque value — interpretation depends on kind and consumer.
    pub value: serde_json::Value,
}

// ── Task ──────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: TaskId,
    pub name: String,
    /// The executor backend to use (e.g. "shell", "kubernetes", "dummy").
    pub executor: String,
    /// Executor-specific configuration.
    pub config: serde_json::Value,
    /// IDs of tasks that must complete before this task runs.
    pub depends_on: Vec<TaskId>,
    /// Artifacts this task consumes as inputs.
    pub inputs: Vec<ArtifactId>,
    /// Artifact IDs this task is expected to produce.
    pub outputs: Vec<ArtifactId>,
}

// ── Work ──────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Work {
    pub id: WorkId,
    pub workspace_id: WorkspaceId,
    pub name: String,
    pub description: Option<String>,
    pub tasks: Vec<Task>,
}

// ── Execution ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskExecutionStatus {
    Pending,
    Running,
    Succeeded,
    Failed(String),
    Cancelled,
    Skipped,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkExecutionStatus {
    Pending,
    Running,
    Succeeded,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskExecution {
    pub task_id: TaskId,
    pub status: TaskExecutionStatus,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub logs: Vec<String>,
    pub outputs: Vec<Artifact>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkExecution {
    pub id: ExecutionId,
    pub work_id: WorkId,
    pub workspace_id: WorkspaceId,
    pub status: WorkExecutionStatus,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub tasks: Vec<TaskExecution>,
}
