# Copilot Task Specifications

_Current date: 2026-03-31_

This document divides `plan.md` items into tasks Copilot can execute autonomously
vs. items that require architect decisions first.  Each Copilot task has a
self-contained spec so work can begin without further discussion.

---

## Global rules

- All produced code must compile with no warnings. To run cargo, use `podman exec -w /orkester/rust/<workspace> orkester-dev cargo check/build/...`
- To run the application, use `podman exec orkester-dev /orkester/dev/build-and-run.sh` (it will recompile and copy binary + plugins, then start the server).
- IMPORTANT ON CODE QUALITY:
   - Follow existing code style and patterns for consistency (e.g. error handling, logging, async patterns).
   - Keep functions small and focused. If a function exceeds ~30 lines, consider refactoring it into smaller helper functions.
   - Keep files organized and modular. If a file grows too large (~250 lines) or contains multiple unrelated components, consider splitting it.
   - Write clear and concise comments, especially for complex logic or public APIs. Avoid redundant comments that just restate the code.
   - Ensure all public functions and structs have doc comments explaining their purpose, parameters, and return values.
   - Use descriptive variable and function names that convey intent. Avoid generic names like `data`, `info`, `handle`, etc.
   - Write unit tests for new functionality and edge cases.
      - Tests should be clear and cover both success and failure scenarios.
      - Tests should not be overly complex or rely on too much setup. Use mocks and fixtures where appropriate to keep them focused.
      - Tests should test the intended behavior and not the implementation details, to allow for refactoring without breaking tests.
   - Avoid code duplication. If you find yourself copying and pasting code, consider abstracting it into a reusable function or module.
   - Use logging effectively to provide insight into the application's behavior, especially for error cases. Ensure that logs are at the appropriate level (e.g. `debug` for detailed internal state, `info` for high-level events, `warn` for recoverable issues, and `error` for critical failures).
   - Ensure that any new dependencies added are justified and do not bloat the project unnecessarily.
   - When modifying existing code, ensure that you understand the current behavior and do not introduce regressions. Run existing tests to confirm that your changes do not break existing functionality.
   - Only modify existing code when necessary for the task. Avoid making unrelated changes to reduce noise in the diff and potential merge conflicts.
- Use the IDE and tools effectively to navigate the codebase, understand types, and find usages. Don't hesitate to read existing code and documentation to understand the context before making changes.
- Don't use shell commands to edit sources (use the IDE). Remember the shell is Powershell, not bash, so syntax is different.
- Never let 'TODO' or 'FIXME' comments in the code unless the task is explicitly about adding a TODO for future work. The code should be production-ready and not contain placeholders.

## Division of Responsibilities

### Architect / Developer

These require design decisions, cross-team coordination, or business context that
Copilot cannot determine alone.

| Plan ref | Item | Why it needs you |
|----------|------|-----------------|
| §2.1 | Global Document / Resource Model ADR | Semver resolution rules, kind taxonomy, schema design |
| §2.3 | Catalog Refactor | Event-sourcing vs snapshot architecture decision |
| §2.5 | Authentication SDK | Auth model, Keycloak realm config, `#[secured]` macro design |
| §2.6 | Authorization SDK | Permission granularity, OPA policy shape |
| §1.2 | Auto-import of Crons | Wiring point decision (see Q1 below) |
| §1.5 | Workaholic YAML tooling for DP | External team coordination |
| §1.3 | DBT Work | Provided by the DP team as a Docker image |
| §3.2 | LocalFsArtifactRegistry | `ArtifactRegistry` trait must be defined first |
| §4.1 | S3 loader crate migration | Decision: AWS crate vs. keep in core (see Q2 below) |
| §7   | `data-platform` plugin | Product / Subscription domain model |
| §9.1 | Input/Output via registries | `ArtifactRegistry` trait and URI scheme must exist first |
| §9.5 | Automatic triggering via `WorkaholicTaskRunner` | Architecture decision |

### Questions for you (Copilot is blocked until answered)

**Q1 — Cron auto-import wiring point (§1.2)**
The `CronScheduler.register()` method exists. Where should the call be made?
- Option A: in `WorkflowServer::start()`, drain all catalog `CronDoc`s at startup,
  then subscribe to a catalog-change event for subsequent adds/removes.
  ***That means adding the ability to subscribe to catalog changes, which may be generally useful for other features too.***
  ***The catalog becomes the global document source, which is a clean architecture. Recommended.***
- Option B: in the document loader action handlers (i.e. when the S3 or LocalFs
  loader publishes a doc whose kind is `workaholic/Cron:1.0`, the server is
  notified via the hub).
  ***This would break the abstraction barrier between the loader and the server -> not recommended***
- Option C: something else?
  ***Another option would be to have some kind of catalog triggers that can run arbitrary code when a doc is added/updated/removed. This would be more flexible but also more complex to implement.***

Response: For now, Option A is the most straightforward and clean solution. It centralizes the cron management in the server and keeps the loaders simple. We can revisit this decision later if we find it limiting.

**Q2 — S3 loader crate home (§4.1)**
The S3 loader implementation (`document/loader/s3/`) is currently in
`orkester-plugin-workaholic`. Should it stay there (for now at least), or move
to `orkester-plugin-workaholic-aws`?  Blocking factor: `aws` crate has an
AWS-specific dependency footprint; moving it avoids pulling AWS deps into the
core plugin for non-AWS deployments.

Response: Move the S3 loader to `orkester-plugin-workaholic-aws` to keep AWS dependencies out of the core plugin. This also makes it clearer that the S3 loader is an AWS-specific component, and allows for better modularity.

**Q3 — HTTP runner response formats (§3.1)**
Should the `HttpTaskRunner` support non-JSON response bodies (e.g. plain text,
raw bytes)? If yes, how should the runner decide which output fields to extract?

Response: The HttpTaskRunner is executing arbitrary HTTP request and retrieving the response. By default the response can be treated as a `file` artifact (dependency on the `ArtifactRegistry` to store it and return a URI). If the response is in a deserializable format (e.g. JSON/YAML/XML) AND the task config specifically opts in to parse it, then we can store it as a structured `data` artifact with extracted fields.

---

## Copilot Tasks — Ready to Implement

Tasks are ordered from highest to lowest value / lowest to highest risk.

---

### TASK-1 · Kubernetes runner: cancel + job hardening `[P2]`

**Plan ref:** §10

**Files touched:**
- `rust/workaholic/crates/orkester-plugin-workaholic/src/workflow/task_runner/kubernetes/run.rs`
- `rust/workaholic/crates/orkester-plugin-workaholic/src/workflow/task_runner/kubernetes/exec.rs`
- `rust/workaholic/crates/orkester-plugin-workaholic/src/workflow/task_runner/kubernetes/job.rs`

**Changes:**

1. **Cancel path deletes the Job.**
   Currently `cancel()` only sets `cancel_requested = true` and sends events;
   the Job is not deleted in the cluster. When `cancel()` is called:
   - Read `state.job_name` under the lock.
   - If `job_name` is `Some`, spawn a detached thread, create a Tokio runtime,
     and call `delete_job(client, namespace, job_name)`.
   - The existing `exec.rs` already deletes after completion — the cancel path
     must do the same thing unconditionally.

2. **Job naming: use full UUID, truncated to 63 chars.**

   - Current job names are generated as `format!("ork-job-{}", &uuid.to_simple().to_string()[..8])`
   - This is not guaranteed to be unique and could cause collisions in long-running systems.
   - It also makes debugging harder because the job name is not directly traceable to the `WorkRun` or `TaskRun`.
   - Prefix should be configurable (e.g. `wh-run-`)
   - Instead of UUID, use the taskRun id/name plus a random suffix if needed to ensure uniqueness.
   - Ensure the full name is truncated to 63 characters to comply with Kubernetes naming rules.
   - Ensure the job name is clearly visible in the logs and events for traceability.

3. **Job labels for observability.**
   In `build_job()` inside `job.rs`, add labels to `ObjectMeta`:
   ```
   workaholic/work-ref        = request.spec.work_ref   (from cfg, pass it in)
   workaholic/task-ref        = request.spec.task_ref   (from cfg, pass it in)
   workaholic/task-run-ref    = ?
   ```

4. **Resource requests and limits config keys.**
   Allow to resources requests and limits for CPU and memory to be set via config keys. This is important for running in shared clusters to avoid resource contention and ensure fair scheduling.

   In `KubeJobConfig` add optional fields:
   ```rust
   pub cpu_request:    Option<String>,   // e.g. "100m"
   pub cpu_limit:      Option<String>,   // e.g. "500m"
   pub memory_request: Option<String>,   // e.g. "128Mi"
   pub memory_limit:   Option<String>,   // e.g. "512Mi"
   ```
   
5. **Allow to mount volumes, secrets and configmaps.**
   
   - Add config keys to specify volumes to mount into the job.
   - Add config keys to specify secrets to mount into the job (mounted as environment variables or files).
   - Add config keys to specify configmaps to mount into the job.

**Definition of done:** `cargo check` passes with no new warnings.

---

### TASK-2 · UI: fix metrics page re-subscribe loop `[P2]`

**Plan ref:** §1.4

**File:** `ui/src/pages/metrics.js`

**Problem:** `renderMetrics()` calls `stopAutoRefresh()` then `startAutoRefresh()` every
time the page is entered (i.e. on every navigation). The refresh timer
accumulates because `startAutoRefresh()` always begins a new `setInterval` but
`stopAutoRefresh()` only clears `_refreshTimer` if it was stored. Inspect the
`start/stopAutoRefresh` functions and confirm this is the case.

**Fix:**
- Read the full `metrics.js` file to verify the exact bug before changing anything.
- Ensure `stopAutoRefresh()` reliably clears the timer before `startAutoRefresh()`
  is called.
- Call `stopAutoRefresh()` in the page's cleanup hook (via `setCleanup` from
  `router.js`) so that navigating away from the page halts polling.

**Definition of done:** Navigating to Metrics, then away, then back does not
result in multiple concurrent polling timers. When on another page, the Metrics polling is halted (can be verified via logs or network tab).

---

### TASK-3 · UI: WorkRun list — use current schema fields `[P2]`

**Plan ref:** §1.4

**File:** `ui/src/pages/workflows.js`

Problems:
1. The page is title / referenced as "Workflows" but it actually shows "Work Runs". This is confusing and should be renamed to "Work Runs".
2. The list currently only shows the "id" (the workRun name) and a status.
3. Other columns are not filled in. Check why (I suspect the wrong field names are used, check the serde_rename in the doc definitions and the API response shape in the server handler).
3. Useful fields like trigger type, trigger time, and step summary are not shown at all.

**Requirements:**
- The first column should be titled "Name".
- Fix the other columns to read from the correct fields in the API response (check the server handler to confirm the shape)
- Add a column with the Trigger (type + time + identity): `wr.spec?.trigger?.trigger_type` + `wr.spec?.trigger?.at` + `wr.spec?.trigger?.identity?`
- Step summary: `wr.status?.summary` — **not shown** (show `succeededSteps/totalSteps` steps as a compact progress indicator in the list row)

**Definition of done:** The list shows the work run name, status, trigger info, and step summary for each work run. The page title is "Work Runs".

---

### TASK-4 · UI: crons list — show next fire time `[P2]`

**Plan ref:** §1.4

**File:** `ui/src/pages/crons.js`

**Current state:** The table already has a "Next run" column that reads
`c.status?.next_scheduled_time` and displays it with `fmtDateShort`. The cell
shows `—` but the field may not be populated. This needs verification.

**Steps:**
1. Read the cron API response shape in the server handler to check whether
   `next_scheduled_time` is actually populated.
2. If the server returns this field but the UI reads the wrong key, fix the key.
3. If the server does not return it: find where `CronDoc` is serialised
   (`WorkflowServer` list-crons handler) and ensure it includes the next
   scheduled time computed by `CronScheduler`.
4. The `CronScheduler` has a `list_crons()` method — check if `CronDoc.status`
   has a `next_scheduled_time` field and whether the scheduler populates it.

**Definition of done:** The "Next run" column shows a real date/time for enabled crons.

---

### TASK-5 · HttpTaskRunner: auth header + retry `[P2]`

**Plan ref:** §3.1

**File:** `rust/workaholic/crates/orkester-plugin-workaholic/src/workflow/task_runner/http.rs`

The current implementation uses a hand-rolled TCP HTTP client (`http_post_json`,
`http_get_status`) which does not support TLS or authentication headers.
The crate already has `ureq` as a dependency — use it instead.

**New config keys to support:**

| Key | Type | Description |
|-----|------|-------------|
| `auth_type` | string | `"bearer"`, `"basic"`, or absent for none |
| `auth_token` | string | Bearer token (when `auth_type = "bearer"`) |
| `auth_user` | string | Username (when `auth_type = "basic"`) |
| `auth_password` | string | Password (when `auth_type = "basic"`) |
| `max_retries` | u64 | Number of retries on transient errors (default 0) |
| `retry_delay_secs` | u64 | Delay between retries in seconds (default 2) |

**Transient errors** (should retry): HTTP 429, 500, 502, 503, 504, and network errors.
**Permanent errors** (should not retry): HTTP 400, 401, 403, 404, any 4xx except 429.

**Implementation notes:**
- Replace `http_post_json` and `http_get_status` with `ureq`-based functions.
- Keep the retry loop inside `run_http_task`, wrapping only the POST call
  (not the poll loop — individual poll failures are already logged and skipped).
- The output extraction stays the same for now: parse JSON body from the poll response,
  extract `status` string and any `outputs` map if present.

**Definition of done:** `cargo check` with no new warnings; existing test-http
config (if any) still compiles.

---

### TASK-6 · Workflow engine: retry logic `[P2]`

**Plan ref:** §9.2

**Files:**
- `rust/workaholic/crates/orkester-plugin-workaholic/src/workflow/work_runner/thread.rs`
- `rust/workaholic/crates/workaholic/src/workflow/` (spec structs — check for retry policy field)

**Design (from plan.md §9.2):**
- Retry policy is at the **step** level inside `WorkRunRequestDoc.spec.steps[*]`.
- Fields: `max_attempts: u32` (default 1, meaning no retry) and
  `retry_delay_secs: u64` (default 0).
- When a step's `TaskRun` finishes with `Failed`, the `ThreadWorkRun` checks
  if `attempts < max_attempts`. If so, it increments `attempts`, waits
  `retry_delay_secs`, and creates a new `TaskRun` from the same frozen
  `TaskRunRequest` (with `attempt` number incremented in the `TaskRunSpec`).
- If `attempts >= max_attempts`, the step is marked `Failed` and the workflow
  proceeds normally (no retry).
- The `WorkRunStepStatus.attempts` field already exists and is tracked.

**Steps:**
1. Check `WorkRunRequestStepSpec` in the `workaholic` crate — does it already have
   `max_attempts` and `retry_delay_secs`? If not, add them with defaults.
2. In `ThreadWorkRun::grant()` (or wherever `TaskRun` completion is handled),
   add the retry check before transitioning the step to `Failed`.
3. Emit `WorkRunEvent::StepStateChanged` with a new transient state `Retrying`
   if one exists, or just re-enter `Running`.

**Definition of done:** `cargo check` with no new warnings. A step configured
with `max_attempts: 3` that fails all three times results in the workflow failing.
A step that fails once and succeeds on retry results in the workflow succeeding.

---

### TASK-7 · Document persistors: wire into server `[P2]`

**Plan ref:** §2.2

**Current state:** `MemoryPersistor` and `LocalFsPersistor` exist and implement
`DocumentPersistor`, but the catalog, workflow state, and metrics server all
use in-memory data structures that are not backed by these components.

**Scope the workflow server state persistence:**
- Find where `Cron`, `WorkRun` and `TaskRun` state is stored in the workflow server.
- Check if it makes sense to also persist `WorkRunRequest` and `TaskRunRequest` (probably yes, for debugging and durability).
- Check if it makes sense to also persist `WorkRunner` and `TaskRunner` state (to be able to recover from a crash of the Workflow server/app).
- Use a `DocumentPersistor` to persist resources instead of in-memory structs.
- The `DocumentPersistor` can be injected into the server at startup, allowing for different implementations (e.g. in-memory for testing, LocalFs for dev, etc).
- Ensure that all reads and writes of workflow state go through the `DocumentPersistor` abstraction.
- The Docs structure already exists in `workaholic` crate — check if it has all the necessary fields for persistence, and add any missing ones.

**Before starting:** Read the workflow server handler (`workflow/server/`) to understand the current in-memory storage model, then confirm the approach.

---

### TASK-8 · REST server: replace raw hyper with axum `[P3]`

**Plan ref:** §8

**Risk:** HIGH — touches all request routing. Do not start until TASK-1 through
TASK-6 are complete and the build is clean.

**Current state:** The HTTP server is built on raw `hyper` with hand-written
routing (likely a match on path components). The hub routes requests to
components via the `HubRoute` config.

**Steps:**
1. Read the current server entry point to understand the routing structure.
2. Create a new `orkester-plugin-core-rest` crate that depends on `axum`.
3. Implement the new Rest Server using Axum.
   - Review the current rest server interface to see if it can be improved with axum's features (e.g. better request parsing, middleware support).
   - Must support static file/folders serving (for example for UI): Should be better than the current basic implementation.
   - Must support internal HUB routing (i.e. forwarding requests to other plugins based on the path and method, as currently configured in `HubRoute`).
   - Must support auto-generated OpenAPI docs (e.g. via `utoipa`) for the internal API routes.
   - Must be secured (AuthN/AuthZ will be added later): SSL/TLS support and CORS support can be added now to prepare for that.
   - Send Metrics events to the HUB.
   - Use Logging (standard Orkester logging macros) extensively (from TRACE to ERROR) to log anything that can be useful for debugging and monitoring the server. Ensure correct levels are used and sensitive information is not logged.
4. Don't migrate existing code, start from scratch in the new crate. The old server can continue running until the new one is ready to swap in. That allows for a more thorough redesign and better use of axum's features, rather than a line-by-line port.
5. Once the new server is implemented and tested it can be loaded and replace the old one in the `workaholic.yaml` config. The `orkester-plugin-sample` crate can be deleted.

**Definition of done:** All existing API endpoints continue to function; the UI
loads correctly; `cargo check` with no new warnings.

---

## Suggested Execution Order

```
TASK-1  (Kubernetes hardening)    — isolated, no dependencies
TASK-2  (Metrics UI fix)          — isolated, UI only
TASK-3  (WorkRun list schema)     — isolated, UI only
TASK-4  (Crons next fire time)    — needs server-side check first
TASK-5  (HttpTaskRunner auth)     — isolated, Rust only
TASK-6  (Retry logic)             — depends on workaholic struct check
TASK-7  (Persistor wiring)        — depends on reading server code
TASK-8  (axum migration)          — last, highest risk
```

Tasks 1–5 can be started immediately.
Tasks 4, 6, 7 need a brief read-first step before implementation.
Task 8 should be deferred until the rest are stable.
