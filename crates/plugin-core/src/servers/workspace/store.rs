//! Typed persistence store — thin wrapper over `PersistenceProvider` that
//! knows about the three Workspace object kinds.
//!
//! Objects are stored under the following key scheme:
//!
//! | Kind      | namespace              | id                        |
//! |-----------|------------------------|---------------------------|
//! | Namespace | `"_namespaces"`        | `<name>`                  |
//! | Task      | `"<ns>/tasks"`         | `<name>/<version>`        |
//! | Work      | `"<ns>/works"`         | `<name>/<version>`        |

use std::sync::Arc;

use orkester_common::plugin::providers::persistence::{EntityKey, PersistenceProvider};
use serde_json::Value;

use super::model::{Namespace, ObjectEnvelope, Task, Work};

pub type StoreResult<T> = Result<T, StoreError>;

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("not found: {0}")]
    NotFound(String),
    #[error("serialization error: {0}")]
    Serialization(String),
    #[error("persistence error: {0}")]
    Persistence(String),
}

impl From<orkester_common::plugin::providers::persistence::PersistenceError> for StoreError {
    fn from(e: orkester_common::plugin::providers::persistence::PersistenceError) -> Self {
        use orkester_common::plugin::providers::persistence::PersistenceError::*;
        match e {
            NotFound(k) => StoreError::NotFound(k),
            Serialization(m) => StoreError::Serialization(m),
            Configuration(m) | Internal(m) => StoreError::Persistence(m),
        }
    }
}

// ── WorkspaceStore ────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct WorkspaceStore {
    inner: Arc<dyn PersistenceProvider>,
}

impl WorkspaceStore {
    pub fn new(provider: Arc<dyn PersistenceProvider>) -> Self {
        Self { inner: provider }
    }

    // ── Namespaces ────────────────────────────────────────────────────────

    pub async fn put_namespace(&self, ns: &Namespace) -> StoreResult<()> {
        let key = ns_key(&ns.meta.name);
        let val = to_value(ns)?;
        self.inner.put(&key, val).await.map_err(Into::into)
    }

    pub async fn get_namespace(&self, name: &str) -> StoreResult<Namespace> {
        let val = self.inner.get(&ns_key(name)).await?;
        from_value(val)
    }

    pub async fn delete_namespace(&self, name: &str) -> StoreResult<()> {
        self.inner.delete(&ns_key(name)).await.map_err(Into::into)
    }

    pub async fn list_namespaces(&self) -> StoreResult<Vec<Namespace>> {
        let ids = self.inner.list("_namespaces").await?;
        let mut out = Vec::with_capacity(ids.len());
        for id in ids {
            let val = self.inner.get(&ns_key(&id)).await?;
            out.push(from_value(val)?);
        }
        Ok(out)
    }

    // ── Tasks ─────────────────────────────────────────────────────────────

    pub async fn put_task(&self, task: &Task) -> StoreResult<()> {
        let key = task_key(&task.meta.metadata.namespace, &task.meta.name, &task.meta.version);
        self.inner.put(&key, to_value(task)?).await.map_err(Into::into)
    }

    pub async fn get_task(&self, namespace: &str, name: &str, version: &str) -> StoreResult<Task> {
        let val = self.inner.get(&task_key(namespace, name, version)).await?;
        from_value(val)
    }

    pub async fn delete_task(&self, namespace: &str, name: &str, version: &str) -> StoreResult<()> {
        self.inner.delete(&task_key(namespace, name, version)).await.map_err(Into::into)
    }

    pub async fn list_tasks(&self, namespace: &str) -> StoreResult<Vec<Task>> {
        let ids = self.inner.list(&format!("{namespace}/tasks")).await?;
        let mut out = Vec::with_capacity(ids.len());
        for id in ids {
            // id is "name/version"
            let (name, version) = split_id(&id);
            let val = self.inner.get(&task_key(namespace, name, version)).await?;
            out.push(from_value(val)?);
        }
        Ok(out)
    }

    // ── Works ─────────────────────────────────────────────────────────────

    pub async fn put_work(&self, work: &Work) -> StoreResult<()> {
        let key = work_key(&work.meta.metadata.namespace, &work.meta.name, &work.meta.version);
        self.inner.put(&key, to_value(work)?).await.map_err(Into::into)
    }

    pub async fn get_work(&self, namespace: &str, name: &str, version: &str) -> StoreResult<Work> {
        let val = self.inner.get(&work_key(namespace, name, version)).await?;
        from_value(val)
    }

    pub async fn delete_work(&self, namespace: &str, name: &str, version: &str) -> StoreResult<()> {
        self.inner.delete(&work_key(namespace, name, version)).await.map_err(Into::into)
    }

    pub async fn list_works(&self, namespace: &str) -> StoreResult<Vec<Work>> {
        let ids = self.inner.list(&format!("{namespace}/works")).await?;
        let mut out = Vec::with_capacity(ids.len());
        for id in ids {
            let (name, version) = split_id(&id);
            let val = self.inner.get(&work_key(namespace, name, version)).await?;
            out.push(from_value(val)?);
        }
        Ok(out)
    }

    // ── Bulk upsert (used by loaders) ─────────────────────────────────────

    pub async fn upsert(&self, obj: &ObjectEnvelope) -> StoreResult<()> {
        match obj {
            ObjectEnvelope::Namespace(n) => self.put_namespace(n).await,
            ObjectEnvelope::Task(t) => self.put_task(t).await,
            ObjectEnvelope::Work(w) => self.put_work(w).await,
        }
    }

    pub async fn remove(&self, obj: &ObjectEnvelope) -> StoreResult<()> {
        match obj {
            ObjectEnvelope::Namespace(n) => self.delete_namespace(&n.meta.name).await,
            ObjectEnvelope::Task(t) => self.delete_task(&t.meta.metadata.namespace, &t.meta.name, &t.meta.version).await,
            ObjectEnvelope::Work(w) => self.delete_work(&w.meta.metadata.namespace, &w.meta.name, &w.meta.version).await,
        }
    }
}

// ── Key helpers ───────────────────────────────────────────────────────────────

fn ns_key(name: &str) -> EntityKey {
    EntityKey { namespace: "_namespaces".into(), id: name.to_string() }
}

fn task_key(namespace: &str, name: &str, version: &str) -> EntityKey {
    let ns = if namespace.is_empty() { "default" } else { namespace };
    EntityKey { namespace: format!("{ns}/tasks"), id: format!("{name}/{version}") }
}

fn work_key(namespace: &str, name: &str, version: &str) -> EntityKey {
    let ns = if namespace.is_empty() { "default" } else { namespace };
    EntityKey { namespace: format!("{ns}/works"), id: format!("{name}/{version}") }
}

fn split_id(id: &str) -> (&str, &str) {
    id.rsplit_once('/').unwrap_or((id, ""))
}

// ── Serde helpers ─────────────────────────────────────────────────────────────

fn to_value<T: serde::Serialize>(v: &T) -> StoreResult<Value> {
    serde_json::to_value(v).map_err(|e| StoreError::Serialization(e.to_string()))
}

fn from_value<T: serde::de::DeserializeOwned>(v: Value) -> StoreResult<T> {
    serde_json::from_value(v).map_err(|e| StoreError::Serialization(e.to_string()))
}
