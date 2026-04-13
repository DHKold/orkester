# UI.md Review — Questions & Remarks

## Gaps & Missing Concepts

### 1. TaskRunners in the Workspace section
UI.md mentions "Runners" as a list of WorkRunners only. The backend has two distinct runner types:
- **WorkRunner** (`workaholic/WorkRunner:1.0`) — orchestrates WorkRuns (user-created, configurable, pooled)
- **TaskRunner** (`workaholic/TaskRunner:1.0`) — executes individual TaskRuns (shell, container, kubernetes, http)

TaskRunners have status, state history and metrics. Should they be exposed in the UI?

> **Decision:** Yes, TaskRunners should be exposed in the UI. They are a critical part of the execution environment and provide valuable information about the system's health and performance. They could be listed alongside WorkRunners in the Runners section, with their own detail pages showing status, history, and metrics. But they should have their own section or tab to avoid confusion, since they serve a different purpose than WorkRunners.

---

### 2. WorkRunnerProfile and TaskRunnerProfile
These catalog documents configure runner behavior (concurrency, timeouts, whitelisted runner kinds). They are currently not mentioned in the Catalog or Workspace sections.

Should they appear in the Catalog section, the Workspace section, or Settings?

> **Decision:** All documents are managed by the catalog. In theory, their could be any kind of document in the catalog, and the UI shouldn't have special cases for certain kinds. So WorkRunnerProfile and TaskRunnerProfile should be listed in the Catalog section, with appropriate filtering and display of relevant fields. They could also be linked from the Workspace section when viewing Runner details, since they directly impact runner behavior.

---

### 3. WorkRunRequest / TaskRunRequest documents
The backend stores requests as separate documents from the runs themselves. "Start a new WorkRun" creates a `WorkRunRequest` which then becomes a `WorkRun`. Should the UI expose these request documents, or abstract them away transparently?

> **Decision:** As any documents, they will show in the Catalog if not filtered out. But they are more of an implementation detail, so the UI should abstract them away in most places. For example, when a user clicks "Run" on a Work, the UI can create the WorkRunRequest behind the scenes and then navigate directly to the new WorkRun details page, without exposing the request document to the user. However, for debugging or advanced use cases, there could be an option to view the underlying request documents in the Catalog with appropriate filtering.

---

### 4. Artifacts / Registry
TaskRun outputs can be artifact references (URIs, e.g. stdout/stderr log files). The backend has a registry with artifact documents. How should the UI render or link to artifacts?

> **Decision:** Artifacts should be accessible from the TaskRun details page, with links or previews depending on the artifact type. The UI could provide a dedicated section for artifacts, showing metadata and allowing users to download or view them directly. For logs, inline viewing or streaming could be supported.

---

### 5. Dashboard widget persistence
The widget system is described as customizable (add/remove/rearrange). Where is this customization stored? If in `core/UserSettings`, that doesn't exist yet in the backend.

> **Decision:** The UI can store widget layout preferences in the browser's local storage for persistence across sessions. This avoids the need for a backend implementation of `core/UserSettings` while still providing a personalized experience. If a backend solution is desired in the future, the UI can be updated to sync with `core/UserSettings` when it becomes available.

---

## Errors in UI.md

### 6. Incorrect `kind` format example
UI.md gives `catalog/Action/Search:2.5` as an example kind. The actual format is `group/Kind:major.minor` (no nested slashes). Real examples: `workaholic/Work:1.0`, `workaholic/Namespace:1.0`.

> **Fix:** In theory, the kind can be anything, the standard format is proposed as a way to avoid collisions. The group can be any string, and can use slashes for a better organizational structure (like `<plugin>/<category>`). So `catalog/Action/Search:2.5` is actually a valid kind, with group `catalog/Action`, kind `Search`, and version `2.5`.

---

### 7. `core/UserSettings` and `core/AppConfig` don't exist
These are described in the Settings section as the backing store for settings, but neither is defined in the backend. Is this intentional future design, or should another mechanism be used?

> **Decision:** The Catalog is able to store any kind of document, even if their kind is not defined in the backend. So `core/UserSettings` and `core/AppConfig` can be used as document kinds for storing settings, even if they don't have specific backend models yet. The UI can create and manage these documents in the Catalog as needed. In a future iteration of the catalog, it will allow defining schemas for automatic validation and better UI rendering, but for now they can be flexible documents with arbitrary fields as needed by the UI.

---

### 8. `core/HelpTopic` doesn't exist
Same as above — presented as existing, but no such document kind is defined in the backend.

> **Decision:** The Catalog is able to store any kind of document, even if their kind is not defined in the backend. So `core/HelpTopic` can be used as a document kind for storing help topics, even if it doesn't have a specific backend model yet. The UI can create and manage these documents in the Catalog as needed. In a future iteration of the catalog, it will allow defining schemas for automatic validation and better UI rendering, but for now they can be flexible documents with arbitrary fields as needed by the UI.

---

## Terminology

### 9. "Tags" vs labels/annotations
UI.md describes metadata tags as key-value pairs. The Rust models have `labels` (which are KV), but `tags` in existing YAML examples are a `Vec<String>` (not KV).

Which model should the UI follow?

> **Decision:** I checked the codebase and it seems that the `tags` field is currently defined as a `Vec<String>` in the Rust models, and this is what is used in the existing YAML examples.

---

## Design Questions

### 10. Authentication & Authorization
The sidebar shows "Logged User" and "team name/id with color" — but there is no auth model in the backend. How is the logged user determined? (e.g., JWT, OIDC proxy header, static config?)

> **Answer:** For now it will be mocked (no real auth), the UI will just use a hardcoded user and team for display purposes. In the future, when an auth model is implemented in the backend, the UI can be updated to fetch the logged user and team information from the backend API, which could be based on JWT tokens, OIDC headers, or any other authentication mechanism that is implemented.

---

### 11. Scope of "Active Namespace"
Does switching the active Namespace in the sidebar filter everything (Catalog + Workspace), or only the Workspace? The Catalog could reasonably show a global cross-namespace view.

> **Answer:** Everything will be scoped to the active Namespace for simplicity. There could be a 'All Namespaces' option in the future if there is demand for a global view, but to start with it will be simpler to have the active Namespace filter both the Catalog and Workspace views. This also encourages users to organize their resources by Namespace for better multi-tenancy and access control.

---

### 12. WorkRun trigger from Catalog
The old UI had a "Run" button directly on Work cards in the Catalog. This flow is not mentioned in UI.md. Should it be included in the new UI?

> **Decision:** Not in the initial version. The catalog will handle documents in a generic way, and won't have special cases for certain kinds. So there won't be a "Run" button directly on Work cards in the Catalog. Instead, users will need to navigate to the Workspace section to create and manage WorkRuns. This keeps the Catalog focused on resource management and avoids mixing in execution actions. Later on, we could define shortcuts or actions for certain document kinds if there is demand for it, but to start with it will be a more uniform experience without special cases.

---

### 13. DAG visualization
The old UI uses Cytoscape.js to render the WorkRun execution graph (step dependencies + live status coloring). This is not mentioned in the Workspace / WorkRun Details description in UI.md. Should it be included?

> **Decision:** Yes, a DAG visualization of the WorkRun execution graph would be a valuable addition to the WorkRun Details page. It provides an intuitive way for users to understand the structure of their workflow, see the dependencies between steps, and quickly identify any failed or running steps through color coding. We can use a library like Cytoscape.js or D3.js to implement this visualization. It should be prominently displayed on the WorkRun Details page, perhaps in a dedicated section below the main metadata and logs, to give users a clear overview of their workflow execution.

---

### 14. TaskRun log display
TaskRun status contains `logsRef.stdout` and `logsRef.stderr` as artifact URIs. How should the new UI fetch and display these logs? (inline, in a panel, via a separate endpoint?)

> **Answer:** The UI should fetch the logs from the artifact URIs provided in `logsRef.stdout` and `logsRef.stderr`. These logs can be displayed inline within the TaskRun details page, perhaps in a collapsible panel or tabbed interface to separate stdout and stderr. This approach allows users to easily view the logs without navigating away from the TaskRun details, while still providing the option to download the logs if needed.

---

### 15. Cron status fields
The Cron backend model has: `last_scheduled_time`, `next_scheduled_time`, `last_run_status`, `consecutive_failures`, `run_count`. None of these are mentioned in the Cron Details page description. Which fields should be surfaced?

> **Answer:** As much as possible should be surfaced to provide users with a clear understanding of their Cron's status and history.

---

### 16. WorkRunner load gauge
WorkRunner status exposes `active_work_runs` and `active_task_runs` — useful as a live load indicator. Should this be shown in the Runners list or detail page?

> **Decision:** Yes, the `active_work_runs` and `active_task_runs` metrics should be displayed in the WorkRunner details page to provide users with insight into the current load and activity of each runner. This information can help users identify which runners are heavily utilized and may need scaling or troubleshooting. It could be displayed prominently at the top of the details page, perhaps with visual indicators (e.g., color coding or gauges) to quickly convey the level of activity.

---
