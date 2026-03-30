//! In-memory registry of WorkRun and TaskRunRequest documents.

use std::collections::HashMap;
use std::sync::Mutex;

use workaholic::{TaskRunDoc, TaskRunRequestDoc, WorkRunDoc, WorkRunRequestDoc};

// ─── Registry ─────────────────────────────────────────────────────────────────

/// Stores frozen requests and live run documents for all active workflow runs.
#[derive(Debug, Default)]
pub struct WorkflowRegistry {
    work_requests:  Mutex<HashMap<String, WorkRunRequestDoc>>,
    task_requests:  Mutex<HashMap<String, TaskRunRequestDoc>>,
    work_runs:      Mutex<HashMap<String, WorkRunDoc>>,
    task_runs:      Mutex<HashMap<String, TaskRunDoc>>,
}

impl WorkflowRegistry {
    pub fn new() -> Self { Self::default() }

    // ── WorkRunRequest ──────────────────────────────────────────────────────

    pub fn insert_work_request(&self, doc: WorkRunRequestDoc) {
        self.work_requests.lock().unwrap().insert(doc.name.clone(), doc);
    }

    pub fn get_work_request(&self, name: &str) -> Option<WorkRunRequestDoc> {
        self.work_requests.lock().unwrap().get(name).cloned()
    }

    // ── TaskRunRequest ──────────────────────────────────────────────────────

    pub fn insert_task_requests(&self, docs: Vec<TaskRunRequestDoc>) {
        let mut map = self.task_requests.lock().unwrap();
        for doc in docs { map.insert(doc.name.clone(), doc); }
    }

    pub fn get_task_request(&self, name: &str) -> Option<TaskRunRequestDoc> {
        self.task_requests.lock().unwrap().get(name).cloned()
    }

    // ── WorkRun ─────────────────────────────────────────────────────────────

    pub fn insert_work_run(&self, doc: WorkRunDoc) {
        self.work_runs.lock().unwrap().insert(doc.name.clone(), doc);
    }

    pub fn update_work_run(&self, doc: WorkRunDoc) {
        self.work_runs.lock().unwrap().insert(doc.name.clone(), doc);
    }

    pub fn get_work_run(&self, name: &str) -> Option<WorkRunDoc> {
        self.work_runs.lock().unwrap().get(name).cloned()
    }

    pub fn list_work_runs(&self) -> Vec<WorkRunDoc> {
        self.work_runs.lock().unwrap().values().cloned().collect()
    }

    // ── TaskRun ──────────────────────────────────────────────────────────────

    pub fn insert_task_run(&self, doc: TaskRunDoc) {
        self.task_runs.lock().unwrap().insert(doc.name.clone(), doc);
    }

    pub fn get_task_run(&self, name: &str) -> Option<TaskRunDoc> {
        self.task_runs.lock().unwrap().get(name).cloned()
    }

    pub fn list_task_runs(&self) -> Vec<TaskRunDoc> {
        self.task_runs.lock().unwrap().values().cloned().collect()
    }
}
