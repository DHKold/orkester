/// Spawn a new `TaskRun` from a frozen `TaskRunRequestDoc`.
/// Payload: `TaskRunRequestDoc`.  Response: `TaskRunDoc`.
pub const ACTION_TASK_RUNNER_SPAWN: &str = "workaholic/TaskRunner/Spawn";

/// Get the current state of a `TaskRunner`.
/// Payload: `()`.  Response: `TaskRunnerDoc`.
pub const ACTION_TASK_RUNNER_GET: &str = "workaholic/TaskRunner/Get";

/// Spawn a new `WorkRun` from a frozen `WorkRunRequestDoc`.
/// Payload: `WorkRunRequestDoc`.  Response: `WorkRunDoc`.
pub const ACTION_WORK_RUNNER_SPAWN: &str = "workaholic/WorkRunner/Spawn";

/// Get the current state of a `WorkRunner`.
/// Payload: `()`.  Response: `WorkRunnerDoc`.
pub const ACTION_WORK_RUNNER_GET: &str = "workaholic/WorkRunner/Get";

/// Trigger a Work execution by name (manual trigger).
/// Payload: `TriggerWorkRequest`.  Response: `WorkRunRequestDoc`.
pub const ACTION_WORKFLOW_TRIGGER: &str = "workaholic/WorkflowServer/Trigger";

/// List all active and recent WorkRuns.
/// Payload: `()`.  Response: `ListWorkRunsResponse`.
pub const ACTION_WORKFLOW_LIST_WORK_RUNS: &str = "workaholic/WorkflowServer/ListWorkRuns";

/// Get a specific WorkRun by name.
/// Payload: `WorkRunRefRequest`.  Response: `WorkRunDoc`.
pub const ACTION_WORKFLOW_GET_WORK_RUN: &str = "workaholic/WorkflowServer/GetWorkRun";

/// Cancel a specific WorkRun by name.
/// Payload: `WorkRunRefRequest`.  Response: `()`.
pub const ACTION_WORKFLOW_CANCEL_WORK_RUN: &str = "workaholic/WorkflowServer/CancelWorkRun";

/// List all TaskRuns (across all WorkRuns).
/// Payload: `()`.  Response: `ListTaskRunsResponse`.
pub const ACTION_WORKFLOW_LIST_TASK_RUNS: &str = "workaholic/WorkflowServer/ListTaskRuns";

/// Get a specific TaskRun by name.
/// Payload: `TaskRunRefRequest`.  Response: `TaskRunDoc`.
pub const ACTION_WORKFLOW_GET_TASK_RUN: &str = "workaholic/WorkflowServer/GetTaskRun";

/// Register a Cron with the workflow server's scheduler.
/// Payload: `CronDoc`.  Response: `()`.
pub const ACTION_WORKFLOW_REGISTER_CRON: &str = "workaholic/WorkflowServer/RegisterCron";

/// Unregister a Cron from the scheduler.
/// Payload: `CronRefRequest`.  Response: `()`.
pub const ACTION_WORKFLOW_UNREGISTER_CRON: &str = "workaholic/WorkflowServer/UnregisterCron";

/// List all registered Crons.
/// Payload: `()`.  Response: `ListCronsResponse`.
pub const ACTION_WORKFLOW_LIST_CRONS: &str = "workaholic/WorkflowServer/ListCrons";
