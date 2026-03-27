# Workflow Server — Implementation Requirements

## Goal

Implement the Workaholic workflow execution system in Rust, split into two crates:

### `workaholic`

Public interfaces, models, traits, and utilities.

Must define:

* `Namespace`
* `Group`
* `Work`
* `Task`
* `Artifact`
* `Cron`
* `WorkRunner`
* `TaskRunner`
* `WorkRunnerProfile`
* `TaskRunnerProfile`
* `WorkRun`
* `TaskRun`
* `DocumentLoader`
* `DocumentParser`
* `DocumentPersistor`

Examples:

* `LocalDocumentLoader`
* `YamlDocumentParser`
* `MemoryDocumentPersistor`
* `LocalFsDocumentPersistor`

### `orkester-plugin-workaholic`

Concrete implementations built on top of the Orkester plugin SDK.

Must contain:

* `WorkflowServer` component
* `WorkRunner` implementations (`LocalWorkRunner`, `ThreadWorkRunner`, `RemoteWorkRunner`, ...)
* `TaskRunner` implementations (`ShellTaskRunner`, `ContainerTaskRunner`, `KubernetesTaskRunner`, ...)

---

## Common document structure

All resource definitions/documents must follow the same base structure.

```yaml
kind: orkester/task:1.0
name: my-task
version: 2.3.5-rc1
metadata:
  description: My task to do some stuff
  author: Myself <...>
spec:
  retry_count: 3
  execution:
    kind: kubernetes
    profile: analytics-default
    config:
      image: ghcr.io/acme/job:1.2.3
      command: ["python", "main.py"]
      args: ["--date", "{{ workflow.date }}"]
```

Rules:

* all declarative resources share the same top-level envelope
* `kind` identifies resource type and schema version
* `version` is the document/resource version and must follow semver
* `metadata` is free-form with standard conventions such as `description` and `tags`
* `spec` contains the resource definition
* runtime/state-bearing resources may also have `status`

The implementation in `workaholic` must define a shared base model/parsing strategy for this envelope.

---

## Responsibilities split

### Workflow Server is responsible for

* creating and managing execution-related objects:

  * `WorkRunner`
  * `TaskRunner`
  * `WorkRun`
  * `TaskRun`
* exposing the execution objects it manages
* sending `Metrics` updates through the HUB
* sending `LogEntry` messages through the HUB
* loading and using a `PersistenceProvider`
* querying the host/components registry through HUB
* creating workRunners/task runners through standard `CreateComponent` calls

### Workflow Server is NOT responsible for

* managing catalog resources:

  * `Namespace`
  * `Group`
  * `Work`
  * `Task`
  * `Cron`
  * `WorkRunnerProfile`
  * `TaskRunnerProfile`

Those are managed by a Catalog Server.

---

## Catalog Server dependency

The Workflow Server must query the Catalog Server for catalog resources.

The Catalog Server is expected to expose handlers like:

* `CreateX`
* `GetX`
* `SetX`
* `DeleteX`
* `ListX`

for each catalog resource.

The Workflow Server must treat the Catalog Server as the source of truth for catalog objects.

---

## Configuration-centric behavior

The Workflow Server is created by the host through `CreateComponent`.

The host passes the Workflow Server configuration as the component config.

The Workflow Server must then use HUB/SDK calls to:

* query available components
* create workRunners
* create task runners
* create/load persistence provider
* interact with logging/metrics services

No direct ad-hoc instantiation outside the plugin/component system.

---

## Execution model

### Workflow Server

Global execution manager.

Responsibilities:

* create workRunners
* queue or assign `WorkRun`
* persist `WorkRun` / `TaskRun`
* publish logs and metrics
* expose execution state

### WorkRunners

WorkRunners are responsible for orchestration of a `WorkRun`.

A workRunner:

* has a queue of `WorkRun`
* may support priorities
* picks a `WorkRun` when it has a free slot
* orchestrates DAG execution
* creates / updates `TaskRun`
* chooses and uses `TaskRunner`
* handles retries
* handles state transitions
* reports logs/metrics/state updates

WorkRunners own orchestration logic.

### TaskRunners

TaskRunners are responsible for actual task execution.

A TaskRunner must:

* execute a `TaskRun`
* inject input `Artifact`s
* extract output `Artifact`s
* monitor execution
* update task state
* produce logs
* report failures

Examples:

* shell process
* local container
* Kubernetes job
* remote execution later

---

## Critical TaskRunner contract

The `TaskRunner` API must be asynchronous by design.

It must **not** be modeled as a simple blocking `run(task)` call.

Required model:

* a `TaskRunner` receives a task execution request
* it starts execution with a `spawn(...)` method
* it returns a handle/object representing the running execution
* the running execution object exposes runtime controls and observation

### Required TaskRunner shape

The exact naming may vary, but the behavior must match this model:

```rust
pub trait TaskRunner {
    fn spawn(&mut self, request: TaskRunRequest) -> Result<RunningTask, TaskRunnerError>;
}
```

The returned running task must expose at least:

* `cancel()`
* `wait()`
* `subscribe()`
* `state()`

Example target shape:

```rust
pub trait RunningTask {
    fn cancel(&mut self) -> Result<(), TaskRunnerError>;
    fn wait(&mut self) -> Result<TaskRunResult, TaskRunnerError>;
    fn subscribe(&mut self) -> Result<TaskRunSubscription, TaskRunnerError>;
    fn state(&self) -> TaskRunState;
}
```

### Required semantics

* `spawn()` starts execution and returns immediately with a handle
* `wait()` blocks or awaits completion of this specific task execution
* `cancel()` requests cancellation of this specific task execution
* `subscribe()` allows receiving runtime updates (logs, state changes, metrics, progress, etc.)
* `state()` returns the latest known local state without blocking

### Why this is mandatory

WorkRunners orchestrate many `TaskRun` concurrently.

So the workRunner must be able to:

* start several task executions
* wait for completion later
* cancel if needed
* observe updates live
* avoid blocking the whole workRunner on one task

A blocking `run()` model is not acceptable for the Workflow Server / WorkRunner architecture.

### TaskRunner ownership model

For V1:

* one `TaskRunner` is created on demand per `TaskRun` attempt
* one spawned execution corresponds to one `TaskRun`
* the task runner and/or running task object must be cleaned up after completion/failure/cancellation

---

## Runtime data ownership

### Managed by Catalog Server

* `Namespace`
* `Group`
* `Work`
* `Task`
* `Cron`
* `WorkRunnerProfile`
* `TaskRunnerProfile`

### Managed by Workflow Server

* `WorkRunner`
* `TaskRunner`
* `WorkRun`
* `TaskRun`

---

## Persistence

The Workflow Server must use a `PersistenceProvider` component.

The PersistenceProvider is used to persist:

* `WorkRun`
* `TaskRun`
* workRunner state if needed
* task runner state if needed
* internal execution metadata if needed

### Critical PersistenceProvider rule

The `PersistenceProvider` must be generic.

It must **not** be defined as a large trait with hardcoded methods such as:

* `getWork()`
* `setWork()`
* `getTask()`
* `setTask()`
* etc.

That design is rejected.

### Required persistence model

Persistence must work on generic resource/documents/objects.

The provider must store and retrieve generic resource values, using the common document envelope.

Expected direction:

* generic object storage
* generic query/list capability
* resource kind/name/version handled as data, not as Rust method names

Example direction:

```rust
pub trait PersistenceProvider {
    fn get(&self, key: ResourceKey) -> Result<Option<ResourceDocument>, PersistenceError>;
    fn put(&self, doc: ResourceDocument) -> Result<(), PersistenceError>;
    fn delete(&self, key: ResourceKey) -> Result<(), PersistenceError>;
    fn list(&self, query: ResourceQuery) -> Result<Vec<ResourceDocument>, PersistenceError>;
}
```

Exact type names may vary, but the design must stay generic.

### Why this is mandatory

The platform is document/configuration centric.

Resources share one common structure, so persistence must be able to store them generically.

Hardcoding one method per resource kind is not acceptable.

---

## HUB / SDK integration

The Workflow Server must use the Orkester SDK/HUB model everywhere.

This means:

* all external interactions go through host/HUB requests
* no direct component-to-component calls
* standard `CreateComponent`, `ListComponents`, etc. must be used

Used for:

* creating workRunners
* creating task runners
* loading persistence provider
* sending logs
* sending metrics
* querying catalog server
* querying component registry

---

## Method granularity requirement

Component methods must be sufficiently granular.

Reason:

* later integration of `#[secured(...)]`

So do not expose only large coarse methods.
Prefer smaller operations like:

* `CreateWorkRun`
* `QueueWorkRun`
* `StartWorkRunner`
* `StopWorkRunner`
* `ExecuteTaskRun`
* `UpdateTaskRunState`
* `PersistWorkRun`
* etc.

Security is not part of this implementation phase, but future compatibility is required.

---

## Error handling requirements

This is critical.

The Workflow Server, WorkRunners, and TaskRunners must:

* never panic in normal runtime conditions
* catch and convert errors to structured failures
* always log failures
* preserve enough context for debugging

A wrong input, bad state, missing component, failed runner, broken persistence provider, or malformed response must:

* not crash the process
* not corrupt execution state silently
* produce logs
* produce failure state updates where relevant

Mandatory rule:

* no `unwrap()` / `expect()` in runtime paths

---

## Logging requirements

The Workflow Server and all execution-related components must send `LogEntry` messages through the HUB.

At minimum log:

* workflow server startup / shutdown
* workRunner creation / deletion / restart
* task runner creation / deletion
* `WorkRun` queued / started / completed / failed / cancelled
* `TaskRun` created / started / retried / completed / failed / cancelled
* persistence failures
* catalog query failures
* workRunner/task runner communication failures
* artifact injection/extraction failures

Each log should include as much context as available:

* `work_run_id`
* `task_run_id`
* `workRunner_id`
* `task_runner_id`
* `work_ref`
* `task_ref`

---

## Metrics requirements

The Workflow Server and related execution components must send metrics updates through the HUB to a Metrics Server.

Track at least:

* active workRunners
* active task runners
* queued work runs
* active work runs
* active task runs
* successful/failed/cancelled work runs
* successful/failed/cancelled task runs
* retry count
* workRunner queue length
* workRunner free slots / capacity
* task execution duration
* work run duration

---

## Resolved decisions

### A. WorkRunner creation policy

Chosen:

* workRunners are created at Workflow Server startup
* workRunners are recreated automatically on failure

### B. TaskRunner creation policy

Chosen:

* task runners are created on demand
* one `TaskRunner` per `TaskRun` attempt

Out of scope for V1:

* pooling
* caching
* runner reuse

### C. WorkRunner selection strategy

Still to define.

Allowed criteria:

* first available
* priority
* matching tags
* load-based

### D. Retry model

Partially resolved.

Chosen:

* retry count source is `Task` / `TaskRef` configuration

Still to define:

* backoff strategy
* retryable vs non-retryable failures

### E. WorkRun queueing model

Chosen:

* one queue per workRunner
* priorities are supported

Still to define:

* exact priority model
* starvation prevention

### F. TaskRun persistence model

Chosen:

* one `TaskRun` per attempt

### G. Recovery model

Partially resolved.

Chosen:

* active `WorkRun` must be recovered after restart
* in-flight `TaskRun` should be re-polled

Still to define:

* exact reconciliation algorithm
* timeout before declaring an in-flight task lost

### H. Cancellation model

Chosen:

* cancellation can be graceful or forced

Still to define:

* graceful timeout
* escalation rule to forced cancellation
* default cancellation mode

---

## Remaining open decisions

### 1. WorkRunner selection strategy

Choose initial default:

* first available
* priority
* matching tags
* load-based

### 2. Retry behavior

Choose:

* backoff strategy
* retryable vs non-retryable failure classification

### 3. Priority model

Choose:

* priority field shape
* ordering rules
* starvation prevention

### 4. Recovery reconciliation

Choose:

* how to rebind recovered `WorkRun` to recreated workRunners
* how long to re-poll in-flight task runs before marking them lost/failed

### 5. Cancellation policy

Choose:

* graceful timeout
* escalation rule to forced cancellation
* default cancellation mode

---

## Minimal internal architecture

Recommended structure:

### In `workaholic`

* `models/`
* `traits/`
* `documents/`
* `persistence/`
* `loading/`
* `parsing/`

### In `orkester-plugin-workaholic`

* `workflow_server/`
* `workRunners/`
* `task_runners/`
* `protocol/`
* `logging/`
* `metrics/`
* `persistence_adapters/`

---

## Minimal implementation flow

Recommended implementation order:

1. define public structs/traits in `workaholic`
2. define protocol messages for workflow execution in `orkester-plugin-workaholic`
3. implement generic `PersistenceProvider` integration
4. implement Workflow Server component skeleton
5. implement one WorkRunner (`LocalWorkRunner`)
6. implement one TaskRunner (`ShellTaskRunner`) with the required async `spawn()` model
7. implement `WorkRun` orchestration
8. implement `TaskRun` execution and retries
9. integrate logs + metrics
10. add more workRunners/runners

---

## Minimum tests required

### Unit tests

* DAG validation
* task dependency resolution
* workRunner queue behavior
* retry logic
* task runner selection
* persistence adapter behavior
* `TaskRunner.spawn()` / `cancel()` / `wait()` / `subscribe()` / `state()` contract

### Integration tests

* create Workflow Server from config
* create WorkRunner through `CreateComponent`
* create TaskRunner through `CreateComponent`
* execute simple one-task `WorkRun`
* execute multi-task DAG
* retry failing task
* persist and reload `WorkRun` / `TaskRun`
* recover active `WorkRun` after restart
* re-poll in-flight `TaskRun`
* log and metric emission

### Failure tests

* missing catalog resource
* persistence provider failure
* workRunner failure
* task runner failure
* artifact injection failure
* malformed HUB response
* invalid config
* bad task runner implementation must not crash workflow server

---

## Deliverable

Implementation is complete when:

* `workaholic` exposes clean public models/traits/interfaces
* `orkester-plugin-workaholic` provides working plugin implementations
* Workflow Server can create and manage workRunners/task runners/work runs/task runs
* TaskRunner contract is asynchronous and supports `spawn()` + running task handle operations
* execution state is persisted through a generic `PersistenceProvider`
* logs and metrics are emitted through HUB
* no normal runtime error can panic the system
* all required tests pass
