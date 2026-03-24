pub mod config;

use std::collections::HashMap;
use std::path::Path;

use orkester_plugin::{prelude::*, sdk::Host};
use serde::{Deserialize, Serialize};
use workaholic::{
    domain::{
        Artifact, Cron, Group, Namespace, Task, TaskRunnerProfile, Work, WorkerProfile,
    },
    loader::LocalDocumentLoader,
    traits::DocumentsLoader,
};

use self::config::CatalogServerConfig;
use crate::host_client::HostClient;

// ── Resource key helper ───────────────────────────────────────────────────────

fn resource_key(namespace: &str, name: &str) -> String {
    format!("{namespace}/{name}")
}

// ── CatalogServer ─────────────────────────────────────────────────────────────

pub struct CatalogServer {
    host: HostClient,
    namespaces: HashMap<String, Namespace>,
    groups: HashMap<String, Group>,
    works: HashMap<String, Work>,
    tasks: HashMap<String, Task>,
    crons: HashMap<String, Cron>,
    worker_profiles: HashMap<String, WorkerProfile>,
    task_runner_profiles: HashMap<String, TaskRunnerProfile>,
    #[allow(dead_code)]
    artifacts: HashMap<String, Artifact>,
}

impl CatalogServer {
    pub fn new(config: CatalogServerConfig, host: Host) -> Self {
        let host = HostClient::new(host);
        let mut server = Self {
            host,
            namespaces: HashMap::new(),
            groups: HashMap::new(),
            works: HashMap::new(),
            tasks: HashMap::new(),
            crons: HashMap::new(),
            worker_profiles: HashMap::new(),
            task_runner_profiles: HashMap::new(),
            artifacts: HashMap::new(),
        };
        if config.load_on_startup {
            if let Some(ref loader_cfg) = config.loader {
                server.load_initial_docs(&loader_cfg.path);
            }
        }
        server
    }

    fn load_initial_docs(&mut self, path: &str) {
        let loader = LocalDocumentLoader;
        let docs = match loader.load(Path::new(path)) {
            Ok(d) => d,
            Err(e) => {
                self.host.log("error", "catalog",
                    &format!("failed to load docs from '{path}': {e}"));
                return;
            }
        };
        let mut counts = [0usize; 8];
        for doc in docs {
            let key = resource_key(&doc.namespace, &doc.name);
            let kind = doc.kind.clone();
            // Re-parse as typed document based on kind prefix.
            if kind.starts_with("orkester/namespace:") {
                match doc.into_typed::<_, ()>() {
                    Ok(v) => { self.namespaces.insert(key, v); counts[0] += 1; }
                    Err(e) => self.host.log("warn", "catalog", &format!("bad namespace doc: {e}")),
                }
            } else if kind.starts_with("orkester/group:") {
                match doc.into_typed::<_, ()>() {
                    Ok(v) => { self.groups.insert(key, v); counts[1] += 1; }
                    Err(e) => self.host.log("warn", "catalog", &format!("bad group doc: {e}")),
                }
            } else if kind.starts_with("orkester/work:") {
                match doc.into_typed::<_, ()>() {
                    Ok(v) => { self.works.insert(key, v); counts[2] += 1; }
                    Err(e) => self.host.log("warn", "catalog", &format!("bad work doc: {e}")),
                }
            } else if kind.starts_with("orkester/task:") {
                match doc.into_typed::<_, ()>() {
                    Ok(v) => { self.tasks.insert(key, v); counts[3] += 1; }
                    Err(e) => self.host.log("warn", "catalog", &format!("bad task doc: {e}")),
                }
            } else if kind.starts_with("orkester/cron:") {
                match doc.into_typed::<_, ()>() {
                    Ok(v) => { self.crons.insert(key, v); counts[4] += 1; }
                    Err(e) => self.host.log("warn", "catalog", &format!("bad cron doc: {e}")),
                }
            } else if kind.starts_with("orkester/workerprofile:") || kind.starts_with("orkester/worker-profile:") {
                match doc.into_typed::<_, ()>() {
                    Ok(v) => { self.worker_profiles.insert(key, v); counts[5] += 1; }
                    Err(e) => self.host.log("warn", "catalog", &format!("bad worker-profile doc: {e}")),
                }
            } else if kind.starts_with("orkester/taskrunnerprofile:") || kind.starts_with("orkester/taskrunner-profile:") {
                match doc.into_typed::<_, ()>() {
                    Ok(v) => { self.task_runner_profiles.insert(key, v); counts[6] += 1; }
                    Err(e) => self.host.log("warn", "catalog", &format!("bad taskrunner-profile doc: {e}")),
                }
            } else {
                self.host.log("debug", "catalog", &format!("skipping unknown kind '{kind}'"));
            }
        }
        self.host.log("info", "catalog", &format!(
            "loaded: {} namespaces, {} groups, {} works, {} tasks, {} crons, {} worker-profiles, {} taskrunner-profiles",
            counts[0], counts[1], counts[2], counts[3], counts[4], counts[5], counts[6]
        ));
    }
}

// ── Request / response messages ───────────────────────────────────────────────

#[derive(Deserialize)]
pub struct ByNameRequest {
    pub namespace: String,
    pub name: String,
}

#[derive(Deserialize)]
pub struct ListRequest {
    #[serde(default)]
    pub namespace: Option<String>,
}

#[derive(Serialize)]
pub struct DeletedResponse {
    pub deleted: bool,
}

// ── Macro-generated PluginComponent impl ─────────────────────────────────────

#[component(
    kind        = "workaholic/CatalogServer:1.0",
    name        = "CatalogServer",
    description = "Manages catalog resources: namespaces, works, tasks, crons, profiles."
)]
impl CatalogServer {
    // ── Namespace ────────────────────────────────────────────────────────────

    #[handle("catalog/CreateNamespace")]
    fn create_namespace(&mut self, doc: Namespace) -> Result<Namespace> {
        let key = resource_key(&doc.namespace, &doc.name);
        if self.namespaces.contains_key(&key) {
            return Err(format!("namespace '{}' already exists in '{}'", doc.name, doc.namespace).into());
        }
        self.host.log("info", "catalog",
            &format!("creating namespace '{}/{}'", doc.namespace, doc.name));
        self.namespaces.insert(key, doc.clone());
        Ok(doc)
    }

    #[handle("catalog/GetNamespace")]
    fn get_namespace(&mut self, req: ByNameRequest) -> Result<Namespace> {
        let key = resource_key(&req.namespace, &req.name);
        self.namespaces
            .get(&key)
            .cloned()
            .ok_or_else(|| format!("namespace '{}' not found in '{}'", req.name, req.namespace).into())
    }

    #[handle("catalog/SetNamespace")]
    fn set_namespace(&mut self, doc: Namespace) -> Result<Namespace> {
        let key = resource_key(&doc.namespace, &doc.name);
        self.host.log("info", "catalog",
            &format!("upsert namespace '{}/{}'", doc.namespace, doc.name));
        self.namespaces.insert(key, doc.clone());
        Ok(doc)
    }

    #[handle("catalog/DeleteNamespace")]
    fn delete_namespace(&mut self, req: ByNameRequest) -> Result<DeletedResponse> {
        let key = resource_key(&req.namespace, &req.name);
        let deleted = self.namespaces.remove(&key).is_some();
        if deleted {
            self.host.log("info", "catalog",
                &format!("deleted namespace '{}/{}'", req.namespace, req.name));
        }
        Ok(DeletedResponse { deleted })
    }

    #[handle("catalog/ListNamespaces")]
    fn list_namespaces(&mut self, req: ListRequest) -> Result<Vec<Namespace>> {
        Ok(self
            .namespaces
            .values()
            .filter(|n| req.namespace.as_deref().map_or(true, |ns| n.namespace == ns))
            .cloned()
            .collect())
    }

    // ── Group ────────────────────────────────────────────────────────────────

    #[handle("catalog/CreateGroup")]
    fn create_group(&mut self, doc: Group) -> Result<Group> {
        let key = resource_key(&doc.namespace, &doc.name);
        if self.groups.contains_key(&key) {
            return Err(format!("group '{}' already exists", doc.name).into());
        }
        self.groups.insert(key, doc.clone());
        Ok(doc)
    }

    #[handle("catalog/GetGroup")]
    fn get_group(&mut self, req: ByNameRequest) -> Result<Group> {
        let key = resource_key(&req.namespace, &req.name);
        self.groups
            .get(&key)
            .cloned()
            .ok_or_else(|| format!("group '{}' not found", req.name).into())
    }

    #[handle("catalog/SetGroup")]
    fn set_group(&mut self, doc: Group) -> Result<Group> {
        let key = resource_key(&doc.namespace, &doc.name);
        self.groups.insert(key, doc.clone());
        Ok(doc)
    }

    #[handle("catalog/DeleteGroup")]
    fn delete_group(&mut self, req: ByNameRequest) -> Result<DeletedResponse> {
        let key = resource_key(&req.namespace, &req.name);
        Ok(DeletedResponse { deleted: self.groups.remove(&key).is_some() })
    }

    #[handle("catalog/ListGroups")]
    fn list_groups(&mut self, req: ListRequest) -> Result<Vec<Group>> {
        Ok(self
            .groups
            .values()
            .filter(|g| req.namespace.as_deref().map_or(true, |ns| g.namespace == ns))
            .cloned()
            .collect())
    }

    // ── Work ─────────────────────────────────────────────────────────────────

    #[handle("catalog/CreateWork")]
    fn create_work(&mut self, doc: Work) -> Result<Work> {
        let key = resource_key(&doc.namespace, &doc.name);
        if self.works.contains_key(&key) {
            return Err(format!("work '{}' already exists in '{}'", doc.name, doc.namespace).into());
        }
        self.works.insert(key, doc.clone());
        Ok(doc)
    }

    #[handle("catalog/GetWork")]
    fn get_work(&mut self, req: ByNameRequest) -> Result<Work> {
        let key = resource_key(&req.namespace, &req.name);
        self.works
            .get(&key)
            .cloned()
            .ok_or_else(|| format!("work '{}' not found in '{}'", req.name, req.namespace).into())
    }

    #[handle("catalog/SetWork")]
    fn set_work(&mut self, doc: Work) -> Result<Work> {
        let key = resource_key(&doc.namespace, &doc.name);
        self.works.insert(key, doc.clone());
        Ok(doc)
    }

    #[handle("catalog/DeleteWork")]
    fn delete_work(&mut self, req: ByNameRequest) -> Result<DeletedResponse> {
        let key = resource_key(&req.namespace, &req.name);
        Ok(DeletedResponse { deleted: self.works.remove(&key).is_some() })
    }

    #[handle("catalog/ListWorks")]
    fn list_works(&mut self, req: ListRequest) -> Result<Vec<Work>> {
        Ok(self
            .works
            .values()
            .filter(|w| req.namespace.as_deref().map_or(true, |ns| w.namespace == ns))
            .cloned()
            .collect())
    }

    // ── Task ─────────────────────────────────────────────────────────────────

    #[handle("catalog/CreateTask")]
    fn create_task(&mut self, doc: Task) -> Result<Task> {
        let key = resource_key(&doc.namespace, &doc.name);
        if self.tasks.contains_key(&key) {
            return Err(format!("task '{}' already exists in '{}'", doc.name, doc.namespace).into());
        }
        self.tasks.insert(key, doc.clone());
        Ok(doc)
    }

    #[handle("catalog/GetTask")]
    fn get_task(&mut self, req: ByNameRequest) -> Result<Task> {
        let key = resource_key(&req.namespace, &req.name);
        self.tasks
            .get(&key)
            .cloned()
            .ok_or_else(|| format!("task '{}' not found in '{}'", req.name, req.namespace).into())
    }

    #[handle("catalog/SetTask")]
    fn set_task(&mut self, doc: Task) -> Result<Task> {
        let key = resource_key(&doc.namespace, &doc.name);
        self.tasks.insert(key, doc.clone());
        Ok(doc)
    }

    #[handle("catalog/DeleteTask")]
    fn delete_task(&mut self, req: ByNameRequest) -> Result<DeletedResponse> {
        let key = resource_key(&req.namespace, &req.name);
        Ok(DeletedResponse { deleted: self.tasks.remove(&key).is_some() })
    }

    #[handle("catalog/ListTasks")]
    fn list_tasks(&mut self, req: ListRequest) -> Result<Vec<Task>> {
        Ok(self
            .tasks
            .values()
            .filter(|t| req.namespace.as_deref().map_or(true, |ns| t.namespace == ns))
            .cloned()
            .collect())
    }

    // ── Cron ─────────────────────────────────────────────────────────────────

    #[handle("catalog/CreateCron")]
    fn create_cron(&mut self, doc: Cron) -> Result<Cron> {
        let key = resource_key(&doc.namespace, &doc.name);
        if self.crons.contains_key(&key) {
            return Err(format!("cron '{}' already exists", doc.name).into());
        }
        self.crons.insert(key, doc.clone());
        Ok(doc)
    }

    #[handle("catalog/GetCron")]
    fn get_cron(&mut self, req: ByNameRequest) -> Result<Cron> {
        let key = resource_key(&req.namespace, &req.name);
        self.crons
            .get(&key)
            .cloned()
            .ok_or_else(|| format!("cron '{}' not found", req.name).into())
    }

    #[handle("catalog/SetCron")]
    fn set_cron(&mut self, doc: Cron) -> Result<Cron> {
        let key = resource_key(&doc.namespace, &doc.name);
        self.crons.insert(key, doc.clone());
        Ok(doc)
    }

    #[handle("catalog/DeleteCron")]
    fn delete_cron(&mut self, req: ByNameRequest) -> Result<DeletedResponse> {
        let key = resource_key(&req.namespace, &req.name);
        Ok(DeletedResponse { deleted: self.crons.remove(&key).is_some() })
    }

    #[handle("catalog/ListCrons")]
    fn list_crons(&mut self, req: ListRequest) -> Result<Vec<Cron>> {
        Ok(self
            .crons
            .values()
            .filter(|c| req.namespace.as_deref().map_or(true, |ns| c.namespace == ns))
            .cloned()
            .collect())
    }

    // ── WorkerProfile ────────────────────────────────────────────────────────

    #[handle("catalog/CreateWorkerProfile")]
    fn create_worker_profile(&mut self, doc: WorkerProfile) -> Result<WorkerProfile> {
        let key = resource_key(&doc.namespace, &doc.name);
        if self.worker_profiles.contains_key(&key) {
            return Err(format!("worker-profile '{}' already exists", doc.name).into());
        }
        self.worker_profiles.insert(key, doc.clone());
        Ok(doc)
    }

    #[handle("catalog/GetWorkerProfile")]
    fn get_worker_profile(&mut self, req: ByNameRequest) -> Result<WorkerProfile> {
        let key = resource_key(&req.namespace, &req.name);
        self.worker_profiles
            .get(&key)
            .cloned()
            .ok_or_else(|| format!("worker-profile '{}' not found", req.name).into())
    }

    #[handle("catalog/SetWorkerProfile")]
    fn set_worker_profile(&mut self, doc: WorkerProfile) -> Result<WorkerProfile> {
        let key = resource_key(&doc.namespace, &doc.name);
        self.worker_profiles.insert(key, doc.clone());
        Ok(doc)
    }

    #[handle("catalog/DeleteWorkerProfile")]
    fn delete_worker_profile(&mut self, req: ByNameRequest) -> Result<DeletedResponse> {
        let key = resource_key(&req.namespace, &req.name);
        Ok(DeletedResponse { deleted: self.worker_profiles.remove(&key).is_some() })
    }

    #[handle("catalog/ListWorkerProfiles")]
    fn list_worker_profiles(&mut self, req: ListRequest) -> Result<Vec<WorkerProfile>> {
        Ok(self
            .worker_profiles
            .values()
            .filter(|p| req.namespace.as_deref().map_or(true, |ns| p.namespace == ns))
            .cloned()
            .collect())
    }

    // ── TaskRunnerProfile ────────────────────────────────────────────────────

    #[handle("catalog/CreateTaskRunnerProfile")]
    fn create_task_runner_profile(&mut self, doc: TaskRunnerProfile) -> Result<TaskRunnerProfile> {
        let key = resource_key(&doc.namespace, &doc.name);
        if self.task_runner_profiles.contains_key(&key) {
            return Err(format!("taskrunner-profile '{}' already exists", doc.name).into());
        }
        self.task_runner_profiles.insert(key, doc.clone());
        Ok(doc)
    }

    #[handle("catalog/GetTaskRunnerProfile")]
    fn get_task_runner_profile(&mut self, req: ByNameRequest) -> Result<TaskRunnerProfile> {
        let key = resource_key(&req.namespace, &req.name);
        self.task_runner_profiles
            .get(&key)
            .cloned()
            .ok_or_else(|| format!("taskrunner-profile '{}' not found", req.name).into())
    }

    #[handle("catalog/SetTaskRunnerProfile")]
    fn set_task_runner_profile(&mut self, doc: TaskRunnerProfile) -> Result<TaskRunnerProfile> {
        let key = resource_key(&doc.namespace, &doc.name);
        self.task_runner_profiles.insert(key, doc.clone());
        Ok(doc)
    }

    #[handle("catalog/DeleteTaskRunnerProfile")]
    fn delete_task_runner_profile(&mut self, req: ByNameRequest) -> Result<DeletedResponse> {
        let key = resource_key(&req.namespace, &req.name);
        Ok(DeletedResponse { deleted: self.task_runner_profiles.remove(&key).is_some() })
    }

    #[handle("catalog/ListTaskRunnerProfiles")]
    fn list_task_runner_profiles(&mut self, req: ListRequest) -> Result<Vec<TaskRunnerProfile>> {
        Ok(self
            .task_runner_profiles
            .values()
            .filter(|p| req.namespace.as_deref().map_or(true, |ns| p.namespace == ns))
            .cloned()
            .collect())
    }
}
