/// This module defines constants for action handles used in the Workaholic application.
/// These constants are used to identify specific actions that can be performed by components within the Workaholic ecosystem.

// === Documents actions ===
pub const DOCUMENTS_LOAD_HANDLE: &str = "workaholic/DocumentsLoader/Load";

// === Persistence management actions ===
pub const PERSISTENCE_GET_HANDLE: &str = "workaholic/Persistence/Get";
pub const PERSISTENCE_PUT_HANDLE: &str = "workaholic/Persistence/Put";
pub const PERSISTENCE_DELETE_HANDLE: &str = "workaholic/Persistence/Delete";
pub const PERSISTENCE_LIST_HANDLE: &str = "workaholic/Persistence/List";

// === Worker actions ===
pub const WORKER_QUEUE_HANDLE: &str = "workaholic/Worker/Queue";
pub const WORKER_UNQUEUE_HANDLE: &str = "workaholic/Worker/Unqueue";

// === TaskRunner actions ===
pub const TASK_RUNNER_RUN_HANDLE: &str = "workaholic/TaskRunner/Run";
pub const TASK_RUNNER_CANCEL_HANDLE: &str = "workaholic/TaskRunner/Cancel";
pub const TASK_RUNNER_STATUS_HANDLE: &str = "workaholic/TaskRunner/Status";
