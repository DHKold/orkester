//! Persistent registry of workflow state, backed by a `DocumentPersistor`.
//!
//! All reads and writes go through the injected `DocumentPersistor`, which can
//! be swapped between an in-memory store (for testing) and a filesystem-based
//! one (for production durability).

use std::sync::Arc;

use serde::{de::DeserializeOwned, Serialize};
use workaholic::{
    CronDoc, DocumentPersistor, EntityValue, PersistorError,
    TaskRunDoc, TaskRunRequestDoc, WorkRunDoc, WorkRunRequestDoc,
};
use orkester_plugin::{log_debug, log_error, log_warn};

const PREFIX_CRONS:         &str = "workflow/crons/";
const PREFIX_WORK_REQUESTS: &str = "workflow/work-requests/";
const PREFIX_TASK_REQUESTS: &str = "workflow/task-requests/";
const PREFIX_WORK_RUNS:     &str = "workflow/work-runs/";
const PREFIX_TASK_RUNS:     &str = "workflow/task-runs/";

/// Persistent store for all live workflow state: crons, work runs, task runs,
/// and the frozen request documents used to create them.
pub struct WorkflowRegistry {
    persistor: Arc<dyn DocumentPersistor>,
}

impl std::fmt::Debug for WorkflowRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WorkflowRegistry").finish()
    }
}

impl WorkflowRegistry {
    /// Create a new registry backed by the given persistor.
    pub fn new(persistor: Arc<dyn DocumentPersistor>) -> Self {
        Self { persistor }
    }

    // ── Cron ────────────────────────────────────────────────────────────────

    /// Persist a cron document, inserting or replacing the entry for its name.
    pub fn upsert_cron(&self, doc: &CronDoc) {
        self.write(PREFIX_CRONS, &doc.name, doc);
    }

    /// Remove a persisted cron by name.  Silently ignores missing entries.
    pub fn remove_cron(&self, name: &str) {
        let key = format!("{PREFIX_CRONS}{name}");
        match self.persistor.delete(&key) {
            Ok(()) | Err(PersistorError::NotFound(_)) => {}
            Err(e) => log_warn!("[registry] delete cron '{name}': {e}"),
        }
    }

    /// Load all persisted cron documents (used to restore state after restart).
    pub fn load_crons(&self) -> Vec<CronDoc> {
        self.list_all(PREFIX_CRONS)
    }

    // ── WorkRunRequest ──────────────────────────────────────────────────────

    /// Persist a frozen `WorkRunRequestDoc`.
    pub fn insert_work_request(&self, doc: WorkRunRequestDoc) {
        self.write(PREFIX_WORK_REQUESTS, &doc.name.clone(), &doc);
    }

    /// Retrieve a `WorkRunRequestDoc` by name.
    pub fn get_work_request(&self, name: &str) -> Option<WorkRunRequestDoc> {
        self.read(PREFIX_WORK_REQUESTS, name)
    }

    // ── TaskRunRequest ──────────────────────────────────────────────────────

    /// Persist a batch of frozen `TaskRunRequestDoc`s (one per step).
    pub fn insert_task_requests(&self, docs: Vec<TaskRunRequestDoc>) {
        for doc in docs { self.write(PREFIX_TASK_REQUESTS, &doc.name.clone(), &doc); }
    }

    /// Retrieve a `TaskRunRequestDoc` by name.
    pub fn get_task_request(&self, name: &str) -> Option<TaskRunRequestDoc> {
        self.read(PREFIX_TASK_REQUESTS, name)
    }

    // ── WorkRun ─────────────────────────────────────────────────────────────

    /// Persist a new `WorkRunDoc`.
    pub fn insert_work_run(&self, doc: WorkRunDoc) {
        self.write(PREFIX_WORK_RUNS, &doc.name.clone(), &doc);
    }

    /// Overwrite an existing `WorkRunDoc` with an updated snapshot.
    pub fn update_work_run(&self, doc: WorkRunDoc) {
        self.write(PREFIX_WORK_RUNS, &doc.name.clone(), &doc);
    }

    /// Retrieve a `WorkRunDoc` by name.
    pub fn get_work_run(&self, name: &str) -> Option<WorkRunDoc> {
        self.read(PREFIX_WORK_RUNS, name)
    }

    /// List all persisted `WorkRunDoc`s.
    pub fn list_work_runs(&self) -> Vec<WorkRunDoc> {
        self.list_all(PREFIX_WORK_RUNS)
    }

    // ── TaskRun ──────────────────────────────────────────────────────────────

    /// Persist a new `TaskRunDoc`.
    pub fn insert_task_run(&self, doc: TaskRunDoc) {
        self.write(PREFIX_TASK_RUNS, &doc.name.clone(), &doc);
    }

    /// Retrieve a `TaskRunDoc` by name.
    pub fn get_task_run(&self, name: &str) -> Option<TaskRunDoc> {
        self.read(PREFIX_TASK_RUNS, name)
    }

    /// List all persisted `TaskRunDoc`s.
    pub fn list_task_runs(&self) -> Vec<TaskRunDoc> {
        self.list_all(PREFIX_TASK_RUNS)
    }

    // ── Private helpers ─────────────────────────────────────────────────────

    fn write<T: Serialize>(&self, prefix: &str, name: &str, doc: &T) {
        let key = format!("{prefix}{name}");
        match to_entity(doc) {
            Ok(entity) => {
                if let Err(e) = self.persistor.put(&key, entity) {
                    log_error!("[registry] write failed for '{key}': {e}");
                } else {
                    log_debug!("[registry] wrote '{key}'");
                }
            }
            Err(e) => log_error!("[registry] serialize failed for '{key}': {e}"),
        }
    }

    fn read<T: DeserializeOwned>(&self, prefix: &str, name: &str) -> Option<T> {
        let key = format!("{prefix}{name}");
        match self.persistor.get(&key) {
            Ok(v)                            => from_entity(v).ok(),
            Err(PersistorError::NotFound(_)) => None,
            Err(e) => {
                log_warn!("[registry] read failed for '{key}': {e}");
                None
            }
        }
    }

    fn list_all<T: DeserializeOwned>(&self, prefix: &str) -> Vec<T> {
        let keys = match self.persistor.list(prefix) {
            Ok(k)  => k,
            Err(e) => { log_warn!("[registry] list failed for '{prefix}': {e}"); return vec![]; }
        };
        keys.iter()
            .filter_map(|k| {
                self.persistor.get(k).ok().and_then(|v| from_entity(v).ok())
            })
            .collect()
    }
}

fn to_entity<T: Serialize>(v: &T) -> Result<EntityValue, serde_json::Error> {
    serde_json::from_value(serde_json::to_value(v)?)
}

fn from_entity<T: DeserializeOwned>(v: EntityValue) -> Result<T, serde_json::Error> {
    serde_json::from_value(serde_json::to_value(v)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::persistor::MemoryPersistor;
    use workaholic::{CronSpec, DocumentMetadata, CRON_KIND};

    fn make_registry() -> WorkflowRegistry {
        WorkflowRegistry::new(Arc::new(MemoryPersistor::new()))
    }

    fn cron_doc(name: &str) -> CronDoc {
        CronDoc {
            kind:     CRON_KIND.to_string(),
            name:     name.to_string(),
            version:  "1.0.0".to_string(),
            metadata: DocumentMetadata { namespace: None, owner: None, description: None, tags: vec![], extra: Default::default() },
            spec:     CronSpec { work_ref: "test:1.0".into(), ..Default::default() },
            status:   None,
        }
    }

    #[test]
    fn cron_upsert_and_list() {
        let reg = make_registry();
        assert!(reg.load_crons().is_empty());

        reg.upsert_cron(&cron_doc("daily"));
        let crons = reg.load_crons();
        assert_eq!(crons.len(), 1);
        assert_eq!(crons[0].name, "daily");
    }

    #[test]
    fn cron_upsert_replaces_existing() {
        let reg = make_registry();
        let mut doc = cron_doc("weekly");
        reg.upsert_cron(&doc);

        doc.spec.work_ref = "other:1.0".into();
        reg.upsert_cron(&doc);

        let crons = reg.load_crons();
        assert_eq!(crons.len(), 1, "upsert must not duplicate");
        assert_eq!(crons[0].spec.work_ref, "other:1.0");
    }

    #[test]
    fn cron_remove() {
        let reg = make_registry();
        reg.upsert_cron(&cron_doc("hourly"));
        reg.remove_cron("hourly");
        assert!(reg.load_crons().is_empty());
    }

    #[test]
    fn cron_remove_missing_is_noop() {
        let reg = make_registry();
        // Should not panic
        reg.remove_cron("nonexistent");
    }

    #[test]
    fn work_run_roundtrip() {
        use workaholic::{Trigger, WORK_RUN_KIND, WorkRunDoc, WorkRunSpec};
        let reg = make_registry();
        let doc = WorkRunDoc {
            kind: WORK_RUN_KIND.to_string(), name: "run-1".into(), version: "1.0.0".into(),
            metadata: DocumentMetadata { namespace: None, owner: None, description: None, tags: vec![], extra: Default::default() },
            spec: WorkRunSpec {
                work_run_request_ref: "req-1".into(), work_ref: "w:1.0".into(),
                work_runner_ref: "r".into(),
                trigger: Trigger { trigger_type: "manual".into(), at: None, identity: None },
            },
            status: None,
        };
        reg.insert_work_run(doc.clone());
        let loaded = reg.get_work_run("run-1").expect("must be found");
        assert_eq!(loaded.name, "run-1");

        let list = reg.list_work_runs();
        assert_eq!(list.len(), 1);
    }
}
