//! Helpers for appending structured log entries to a WorkRun's status.

use chrono::Utc;
use workaholic::WorkRunLogEntry;

use super::registry::WorkflowRegistry;

/// Append a structured log entry to the WorkRun identified by `run`.
/// Silently no-ops if the WorkRun or its status cannot be found.
pub fn append_run_log(registry: &WorkflowRegistry, run: &str, level: &str, message: String) {
    if let Some(mut doc) = registry.get_work_run(run) {
        if let Some(status) = doc.status.as_mut() {
            status.logs.push(WorkRunLogEntry {
                ts:      Utc::now().to_rfc3339(),
                level:   level.to_string(),
                message,
            });
        }
        registry.update_work_run(doc);
    }
}
