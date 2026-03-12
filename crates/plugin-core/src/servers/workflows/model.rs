//! Domain model for the Workflows server.
//!
//! Two first-class resources:
//!
//! * [`Workflow`] — a running (or historical) instance of a [`Work`] definition.
//! * [`Cron`]     — a schedule that creates Workflow instances automatically.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

// ── WorkflowStatus ────────────────────────────────────────────────────────────

/// Lifecycle state of a Workflow instance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowStatus {
    /// Created but waiting for its `start_condition` to be met or
    /// `start_datetime` to arrive.
    #[default]
    Waiting,
    /// Actively executing steps.
    Running,
    /// Execution has been suspended; can be resumed.
    Paused,
    /// All steps completed successfully.
    Succeeded,
    /// One or more steps failed and the failure was not ignored.
    Failed,
    /// Cancelled by a user or by a Cron policy before or during execution.
    Cancelled,
}

impl WorkflowStatus {
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Succeeded | Self::Failed | Self::Cancelled)
    }

    pub fn is_active(&self) -> bool {
        matches!(self, Self::Waiting | Self::Running | Self::Paused)
    }
}

impl std::fmt::Display for WorkflowStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = serde_json::to_value(self)
            .ok()
            .and_then(|v| v.as_str().map(str::to_string))
            .unwrap_or_else(|| format!("{:?}", self));
        f.write_str(&s)
    }
}

// ── StepStatus ────────────────────────────────────────────────────────────────

/// Execution state of a single step within a Workflow.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepStatus {
    Pending,
    Running,
    Succeeded,
    Failed,
    Skipped,
    Cancelled,
}

// ── FailurePolicy ─────────────────────────────────────────────────────────────

/// How the Workflow reacts when a step fails.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FailurePolicy {
    /// Stop immediately and mark the Workflow as Failed (default).
    FailFast,
    /// Continue running independent steps, then mark the Workflow as Failed.
    ContinueOnFailure,
    /// Treat any step failure as success (useful for optional side-effects).
    IgnoreFailures,
}

impl Default for FailurePolicy {
    fn default() -> Self {
        Self::FailFast
    }
}

// ── Workflow ──────────────────────────────────────────────────────────────────

/// A single execution instance of a Work definition.
///
/// ```yaml
/// id: "550e8400-e29b-41d4-a716-446655440000"
/// namespace: data-platform
/// work_name: daily-etl-pipeline
/// work_version: "1.0.0"
/// work_context:
///   source_date: "2026-03-12"
///   source_bucket: my-raw-bucket
///   target_table: events_curated
/// schedule:
///   start_datetime: "2026-03-13T00:00:00Z"
///   start_condition: null
/// execution:
///   failure_policy: fail_fast
/// status: waiting
/// triggers:
///   cron_id: "nightly-etl"
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workflow {
    /// Unique identifier (UUID v4). Stamped by the server if omitted.
    #[serde(default)]
    pub id: String,
    /// Namespace that owns this workflow. Overwritten by the server from the URL.
    #[serde(default)]
    pub namespace: String,

    // ── What to run ───────────────────────────────────────────────────────
    /// Name of the Work definition to execute.
    pub work_name: String,
    /// Version of the Work definition to execute.
    pub work_version: String,
    /// Input values passed to the Work at runtime (name → value).
    #[serde(default)]
    pub work_context: HashMap<String, Value>,

    // ── When to run ───────────────────────────────────────────────────────
    #[serde(default)]
    pub schedule: WorkflowSchedule,

    // ── How to run ────────────────────────────────────────────────────────
    #[serde(default)]
    pub execution: WorkflowExecution,

    // ── Trigger provenance ────────────────────────────────────────────────
    /// Populated when the Workflow was created by a Cron.
    #[serde(default)]
    pub triggers: WorkflowTriggers,

    // ── State ─────────────────────────────────────────────────────────────
    #[serde(default)]
    pub status: WorkflowStatus,
    #[serde(default)]
    pub steps: HashMap<String, StepState>,
    #[serde(default)]
    pub metrics: WorkflowMetrics,

    #[serde(default)]
    pub created_at: DateTime<Utc>,
    #[serde(default)]
    pub updated_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
}

impl Workflow {
    pub fn new(
        namespace: impl Into<String>,
        work_name: impl Into<String>,
        work_version: impl Into<String>,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            namespace: namespace.into(),
            work_name: work_name.into(),
            work_version: work_version.into(),
            work_context: HashMap::new(),
            schedule: WorkflowSchedule::default(),
            execution: WorkflowExecution::default(),
            triggers: WorkflowTriggers::default(),
            status: WorkflowStatus::Waiting,
            steps: HashMap::new(),
            metrics: WorkflowMetrics::default(),
            created_at: now,
            updated_at: now,
            started_at: None,
            finished_at: None,
        }
    }
}

/// Scheduling parameters: when should this Workflow begin execution.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorkflowSchedule {
    /// If set, the Workflow will not start before this UTC timestamp.
    pub start_datetime: Option<DateTime<Utc>>,
    /// Arbitrary expression evaluated by the Worker before starting
    /// (e.g. a sensor condition).  `null` means start immediately.
    pub start_condition: Option<String>,
}

/// Execution policy parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowExecution {
    #[serde(default)]
    pub failure_policy: FailurePolicy,
    /// Maximum wall-clock time before the whole Workflow is considered Failed.
    pub timeout_seconds: Option<u64>,
}

impl Default for WorkflowExecution {
    fn default() -> Self {
        Self {
            failure_policy: FailurePolicy::default(),
            timeout_seconds: None,
        }
    }
}

/// Provenance: who/what triggered this Workflow.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorkflowTriggers {
    /// ID of the Cron that created this Workflow, if any.
    pub cron_id: Option<String>,
}

/// Per-step execution state stored inside the Workflow.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepState {
    pub status: StepStatus,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    /// Output values produced by this step.
    #[serde(default)]
    pub outputs: HashMap<String, Value>,
    /// Error message if the step failed.
    pub error: Option<String>,
    /// Number of attempts made (1-based).
    #[serde(default = "default_one")]
    pub attempt: u32,
}

fn default_one() -> u32 { 1 }

/// Aggregate execution metrics for a Workflow.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorkflowMetrics {
    pub steps_total: u32,
    pub steps_succeeded: u32,
    pub steps_failed: u32,
    pub steps_skipped: u32,
    /// Wall-clock duration in seconds (set when the Workflow finishes).
    pub duration_seconds: Option<f64>,
}

// ── CronConcurrencyPolicy ─────────────────────────────────────────────────────

/// What a Cron does when it fires and an existing active Workflow already
/// exists for the same Work, keyed by the existing Workflow's status.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConcurrencyAction {
    /// Create a new Workflow regardless of the existing one.
    Allow,
    /// Skip this firing; keep the existing Workflow running.
    Skip,
    /// Cancel the existing Workflow and start a fresh one.
    Replace,
    /// Cancel the existing Workflow and do NOT start a new one.
    CancelExisting,
}

impl Default for ConcurrencyAction {
    fn default() -> Self {
        Self::Skip
    }
}

/// Fine-grained concurrency policy: different actions per existing status.
///
/// Any status not listed falls back to `default_action`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CronConcurrencyPolicy {
    /// Action when an existing Workflow is in Waiting state.
    #[serde(default)]
    pub on_waiting: ConcurrencyAction,
    /// Action when an existing Workflow is Running.
    #[serde(default)]
    pub on_running: ConcurrencyAction,
    /// Action when an existing Workflow is Paused.
    #[serde(default)]
    pub on_paused: ConcurrencyAction,
    /// Fall-through action for any other status (terminal statuses).
    #[serde(default)]
    pub default_action: ConcurrencyAction,
}

impl Default for CronConcurrencyPolicy {
    fn default() -> Self {
        Self {
            on_waiting: ConcurrencyAction::Skip,
            on_running: ConcurrencyAction::Skip,
            on_paused: ConcurrencyAction::Skip,
            default_action: ConcurrencyAction::Allow,
        }
    }
}

// ── Cron ─────────────────────────────────────────────────────────────────────

/// A time-based trigger that creates Workflow instances on a schedule.
///
/// ```yaml
/// id: nightly-etl
/// namespace: data-platform
/// description: "Run the daily ETL pipeline every night at 01:00 UTC"
/// schedule: "0 1 * * *"
/// enabled: true
/// work_name: daily-etl-pipeline
/// work_version: "1.0.0"
/// work_context:
///   source_bucket: my-raw-bucket
///   target_table: events_curated
/// concurrency_policy:
///   on_running: skip
///   on_waiting: replace
///   default_action: allow
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cron {
    /// User-chosen unique identifier within the namespace (e.g. `"nightly-etl"`). Stamped by the server if omitted.
    #[serde(default)]
    pub id: String,
    /// Overwritten by the server from the URL.
    #[serde(default)]
    pub namespace: String,
    #[serde(default)]
    pub description: String,

    // ── Schedule ──────────────────────────────────────────────────────────
    /// Unix-style cron expression: `"<min> <hour> <dom> <mon> <dow>"`.
    /// Standard 5-field syntax; seconds are not supported.
    pub schedule: String,
    /// When `false`, the cron will not fire even if its time arrives.
    #[serde(default = "default_true")]
    pub enabled: bool,

    // ── What to create ────────────────────────────────────────────────────
    /// Work definition to instantiate.
    pub work_name: String,
    pub work_version: String,
    /// Default input values forwarded to every Workflow created by this Cron.
    #[serde(default)]
    pub work_context: HashMap<String, Value>,
    #[serde(default)]
    pub execution: WorkflowExecution,

    // ── Concurrency policy ────────────────────────────────────────────────
    #[serde(default)]
    pub concurrency_policy: CronConcurrencyPolicy,

    // ── Metadata ──────────────────────────────────────────────────────────
    #[serde(default)]
    pub created_at: DateTime<Utc>,
    #[serde(default)]
    pub updated_at: DateTime<Utc>,
    /// Last time this Cron fired (i.e. attempted to create a Workflow).
    pub last_fired_at: Option<DateTime<Utc>>,
    /// Next scheduled firing time (pre-computed by the scheduler).
    pub next_fire_at: Option<DateTime<Utc>>,
}

impl Cron {
    pub fn new(
        id: impl Into<String>,
        namespace: impl Into<String>,
        schedule: impl Into<String>,
        work_name: impl Into<String>,
        work_version: impl Into<String>,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: id.into(),
            namespace: namespace.into(),
            description: String::new(),
            schedule: schedule.into(),
            enabled: true,
            work_name: work_name.into(),
            work_version: work_version.into(),
            work_context: HashMap::new(),
            execution: WorkflowExecution::default(),
            concurrency_policy: CronConcurrencyPolicy::default(),
            created_at: now,
            updated_at: now,
            last_fired_at: None,
            next_fire_at: None,
        }
    }

    /// Compute the next fire time after `after`.
    ///
    /// TODO: integrate the `cron` crate for proper expression parsing.
    /// For now advances by 1 minute as a safe stub.
    pub fn next_fire_after(schedule: &str, after: DateTime<Utc>) -> Option<DateTime<Utc>> {
        let _ = schedule;
        Some(after + chrono::Duration::minutes(1))
    }
}

fn default_true() -> bool { true }
