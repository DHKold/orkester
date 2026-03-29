//! Catalog client - queries the in-process CatalogServer via host action dispatch.
//!
//! NOTE: Use ONLY from background threads, never from inside a component handler.
//! Calling host.handle() from within a handler deadlocks the pipeline worker.

use std::{collections::HashMap, sync::atomic::{AtomicU64, Ordering}};
use workaholic::{TaskDoc, WorkDoc, WorkaholicError, WORK_KIND};

use orkester_plugin::hub::Envelope;

use crate::catalog::actions::{ACTION_CATALOG_LIST_TASKS, ACTION_CATALOG_RETRIEVE_RESOURCE};
use crate::catalog::request::ListItemsRequest;

static NEXT_ENVELOPE_ID: AtomicU64 = AtomicU64::new(10_000);
fn next_id() -> u64 { NEXT_ENVELOPE_ID.fetch_add(1, Ordering::Relaxed) }

// --- CatalogClient -----------------------------------------------------------

pub struct CatalogClient {
    host: orkester_plugin::sdk::Host,
}

impl CatalogClient {
    pub fn new(host: orkester_plugin::sdk::Host, _catalog_ref: impl Into<String>) -> Self {
        Self { host }
    }

    /// Load a WorkDoc from the catalog by its work ref (namespace/name).
    pub fn get_work(&mut self, work_ref: &str) -> Result<WorkDoc, WorkaholicError> {
        let id = normalize_id(WORK_KIND, work_ref);
        eprintln!("[catalog-client] get_work id='{id}'");

        let payload = serde_json::to_vec(&serde_json::json!({ "id": id })).unwrap_or_default();
        let envelope = Envelope { id: next_id(), owner: None, kind: ACTION_CATALOG_RETRIEVE_RESOURCE.to_string(), format: "std/json".to_string(), payload };
        let resp: serde_json::Value = self.host.handle(&envelope)
            .map_err(|e| WorkaholicError::Other(e.to_string()))?;

        eprintln!("[catalog-client] get_work resp={resp}");

        let doc_val = first_response(&resp)
            .ok_or_else(|| WorkaholicError::NotFound { kind: "Work".into(), name: work_ref.into() })?;

        serde_json::from_value(doc_val).map_err(WorkaholicError::Json)
    }

    /// Load all TaskDocs in namespace and index by multiple key formats.
    pub fn get_tasks_by_ref(&mut self, namespace: &str) -> Result<HashMap<String, TaskDoc>, WorkaholicError> {
        let payload = serde_json::to_vec(&ListItemsRequest { ns: namespace.to_string() }).unwrap_or_default();
        let envelope = Envelope { id: next_id(), owner: None, kind: ACTION_CATALOG_LIST_TASKS.to_string(), format: "std/json".to_string(), payload };
        let resp: serde_json::Value = self.host.handle(&envelope)
            .map_err(|e| WorkaholicError::Other(e.to_string()))?;

        eprintln!("[catalog-client] list_tasks resp={resp}");

        let list  = first_response(&resp).unwrap_or_default();
        let tasks: Vec<TaskDoc> = list
            .get("tasks")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();

        eprintln!("[catalog-client] deserialized {} task(s) in namespace '{namespace}'", tasks.len());
        Ok(index_tasks(tasks))
    }
}

// --- Helpers -----------------------------------------------------------------

/// Extract the first element from resp["responses"] (the pipeline response wrapper).
fn first_response(resp: &serde_json::Value) -> Option<serde_json::Value> {
    resp.get("responses")
        .and_then(|r| r.as_array())
        .and_then(|a| a.first())
        .cloned()
}

/// Index tasks by multiple key formats so any task_ref lookup succeeds.
///
/// Supported lookups: "name", "name:version", "ns/name", "ns/name:version".
fn index_tasks(tasks: Vec<TaskDoc>) -> HashMap<String, TaskDoc> {
    let mut map = HashMap::new();
    for t in tasks {
        let ns = t.metadata.namespace.as_deref().unwrap_or("global");
        let keys = [
            t.name.clone(),
            format!("{}:{}", t.name, t.version),
            format!("{}/{}", ns, t.name),
            format!("{}/{}:{}", ns, t.name, t.version),
        ];
        for key in &keys {
            map.insert(key.clone(), t.clone());
        }
    }
    map
}

/// Convert "namespace/name[:version]" to "kind/namespace/name/version".
fn normalize_id(kind: &str, work_ref: &str) -> String {
    let (ns_name, version) = work_ref
        .split_once(':')
        .map(|(l, r)| (l, r.to_string()))
        .unwrap_or((work_ref, "1.0".to_string()));
    let (ns, name) = ns_name
        .split_once('/')
        .map(|(l, r)| (l, r))
        .unwrap_or(("global", ns_name));
    format!("{}/{}/{}/{}", kind, ns, name, version)
}
