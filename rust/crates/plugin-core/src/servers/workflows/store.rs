//! Typed persistence store for the Workflows server.
//!
//! Key scheme:
//!
//! | Kind     | namespace              | id                                   |
//! |----------|------------------------|--------------------------------------|
//! | Workflow | `"<ns>/workflows"`     | `<workflow_id>`                      |
//! | Cron     | `"<ns>/crons"`         | `<cron_id>`                          |

use std::sync::Arc;

use orkester_common::plugin::providers::persistence::{EntityKey, PersistenceProvider};
use serde_json::Value;

use super::model::{Cron, Workflow};

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

// ── WorkflowsStore ────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct WorkflowsStore {
    inner: Arc<dyn PersistenceProvider>,
}

impl WorkflowsStore {
    pub fn new(provider: Arc<dyn PersistenceProvider>) -> Self {
        Self { inner: provider }
    }

    // ── Workflows ─────────────────────────────────────────────────────────

    pub async fn put_workflow(&self, wf: &Workflow) -> StoreResult<()> {
        let key = workflow_key(&wf.namespace, &wf.id);
        self.inner
            .put(&key, to_value(wf)?)
            .await
            .map_err(Into::into)
    }

    pub async fn get_workflow(&self, namespace: &str, id: &str) -> StoreResult<Workflow> {
        let val = self.inner.get(&workflow_key(namespace, id)).await?;
        from_value(val)
    }

    pub async fn delete_workflow(&self, namespace: &str, id: &str) -> StoreResult<()> {
        self.inner
            .delete(&workflow_key(namespace, id))
            .await
            .map_err(Into::into)
    }

    pub async fn list_workflows(&self, namespace: &str) -> StoreResult<Vec<Workflow>> {
        let ids = self.inner.list(&format!("{namespace}/workflows")).await?;
        let mut out = Vec::with_capacity(ids.len());
        for id in ids {
            let val = self.inner.get(&workflow_key(namespace, &id)).await?;
            out.push(from_value(val)?);
        }
        Ok(out)
    }

    /// Returns all active (non-terminal) workflows for a given Work definition.
    pub async fn list_active_workflows_for_work(
        &self,
        namespace: &str,
        work_name: &str,
        work_version: &str,
    ) -> StoreResult<Vec<Workflow>> {
        let all = self.list_workflows(namespace).await?;
        Ok(all
            .into_iter()
            .filter(|wf| {
                wf.work_name == work_name
                    && wf.work_version == work_version
                    && wf.status.is_active()
            })
            .collect())
    }

    // ── Crons ─────────────────────────────────────────────────────────────

    pub async fn put_cron(&self, cron: &Cron) -> StoreResult<()> {
        let key = cron_key(&cron.namespace, &cron.id);
        self.inner
            .put(&key, to_value(cron)?)
            .await
            .map_err(Into::into)
    }

    pub async fn get_cron(&self, namespace: &str, id: &str) -> StoreResult<Cron> {
        let val = self.inner.get(&cron_key(namespace, id)).await?;
        from_value(val)
    }

    pub async fn delete_cron(&self, namespace: &str, id: &str) -> StoreResult<()> {
        self.inner
            .delete(&cron_key(namespace, id))
            .await
            .map_err(Into::into)
    }

    pub async fn list_crons(&self, namespace: &str) -> StoreResult<Vec<Cron>> {
        let ids = self.inner.list(&format!("{namespace}/crons")).await?;
        let mut out = Vec::with_capacity(ids.len());
        for id in ids {
            let val = self.inner.get(&cron_key(namespace, &id)).await?;
            out.push(from_value(val)?);
        }
        Ok(out)
    }

    /// Returns all enabled crons across *all* namespaces.
    ///
    /// Used by the scheduler loop to find crons that are due to fire.
    pub async fn list_all_enabled_crons(&self) -> StoreResult<Vec<Cron>> {
        // The current PersistenceProvider does not support cross-namespace scans,
        // so we keep a special index namespace that stores cron IDs as
        // "<namespace>/<id>" strings.
        let ids = self.inner.list("_cron_index").await.unwrap_or_default();
        let mut out = Vec::new();
        for composite_id in ids {
            if let Some((ns, id)) = composite_id.split_once('/') {
                match self.get_cron(ns, id).await {
                    Ok(c) if c.enabled => out.push(c),
                    Ok(_) => {}
                    Err(_) => {}
                }
            }
        }
        Ok(out)
    }

    /// Maintain the global cron index when a cron is created/updated.
    pub async fn index_cron(&self, cron: &Cron) -> StoreResult<()> {
        let key = EntityKey {
            namespace: "_cron_index".to_string(),
            id: format!("{}/{}", cron.namespace, cron.id),
        };
        self.inner
            .put(&key, serde_json::Value::Bool(true))
            .await
            .map_err(Into::into)
    }

    /// Remove a cron from the global index.
    pub async fn deindex_cron(&self, namespace: &str, id: &str) -> StoreResult<()> {
        let key = EntityKey {
            namespace: "_cron_index".to_string(),
            id: format!("{namespace}/{id}"),
        };
        self.inner.delete(&key).await.map_err(Into::into)
    }
}

// ── Key helpers ───────────────────────────────────────────────────────────────

fn workflow_key(namespace: &str, id: &str) -> EntityKey {
    EntityKey {
        namespace: format!("{namespace}/workflows"),
        id: id.to_string(),
    }
}

fn cron_key(namespace: &str, id: &str) -> EntityKey {
    EntityKey {
        namespace: format!("{namespace}/crons"),
        id: id.to_string(),
    }
}

// ── Serialisation helpers ─────────────────────────────────────────────────────

fn to_value<T: serde::Serialize>(v: &T) -> StoreResult<Value> {
    serde_json::to_value(v).map_err(|e| StoreError::Serialization(e.to_string()))
}

fn from_value<T: serde::de::DeserializeOwned>(v: Value) -> StoreResult<T> {
    serde_json::from_value(v).map_err(|e| StoreError::Serialization(e.to_string()))
}
