# Workflow Engine Specification

This document defines the architecture, responsibilities, resource model, lifecycle, and Rust trait interfaces for the workflow engine.

It is optimized as implementation input for code generation tools. It is intentionally explicit about ownership, boundaries, and forbidden ambiguities.

## 1. Goals

The engine must support:

* reusable workflow and task definitions
* immutable, frozen execution requests
* live execution handles for workflows and tasks
* global orchestration and resource management
* multiple task execution backends
* deterministic execution, including retries
* exportable/importable document representations for all major runtime objects
* event-driven orchestration
* cron-based and manual triggering

The engine must preserve consistency from the moment a workflow execution is resolved. Editing or deleting a `Work` or `Task` after request creation must not affect already created execution requests.

## 2. Core Concepts

There are four categories of objects.

### 2.1 Reusable Definitions

Reusable definitions are user-authored specifications.

* `Work`
* `Task`

They define abstract execution behavior.

### 2.2 Frozen Execution Requests

Frozen execution requests are immutable snapshots resolved before execution starts.

* `WorkRunRequest`
* `TaskRunRequest`

They contain everything required to execute deterministically.

### 2.3 Live Runtime Handles

Live runtime handles represent currently running or completed executions.

* `WorkRun`
* `TaskRun`

They expose behavior such as start, cancel, subscribe, and document export.

### 2.4 Execution Engines

Execution engines create and supervise runtime handles.

* `WorkRunner`
* `TaskRunner`

`WorkRunner` runs workflows.
`TaskRunner` runs tasks.

## 3. High-Level Execution Flow

### 3.1 Resolution Flow

1. A `Trigger` is created, manually or automatically from a `Cron`.
2. A `TriggerResolver` resolves the trigger into:

   * one frozen `WorkRunRequest`
   * all frozen `TaskRunRequest`s for the workflow steps
3. The resolved requests are stored in the `WorkRunner` local registry.

### 3.2 Runtime Flow

1. When the `WorkRunner` has a `work_permit`, it selects the next `WorkRunRequest`.
2. It spawns a `WorkRun` from that request.
3. It subscribes to the `WorkRun` events.
4. It starts the `WorkRun`.
5. The `WorkRunner` grants resources to the `WorkRun`.
6. The `WorkRun` uses those resources to start `TaskRun`s from precomputed `TaskRunRequest`s.
7. Each `TaskRun` emits events.
8. The `WorkRun` uses `TaskRun` events to orchestrate the DAG.
9. The `WorkRunner` uses `WorkRun` and `TaskRun` events to track metrics, resource usage, scheduling, cancellation, and quotas.

## 4. Responsibilities

### 4.1 Trigger

A `Trigger` is a lightweight execution intent.

It contains only the minimum required information to request workflow resolution, such as:

* `workRef`
* input overrides
* additional execution policy such as concurrency or failure policy
* identity / origin / source metadata

A `Trigger` does not resolve tasks, artifacts, defaults, or mappings.

### 4.2 TriggerResolver

The `TriggerResolver` is responsible for producing frozen execution requests.

It must:

* load the referenced `Work`
* load every referenced `Task`
* resolve all workflow inputs
* resolve all step input mappings
* resolve all output destinations
* freeze all execution configurations
* produce one `WorkRunRequest`
* produce one `TaskRunRequest` per logical step

The result must be deterministic and self-consistent.

### 4.3 WorkRunner

The `WorkRunner` is the global workflow execution engine.

It must:

* maintain a local registry of pending and active requests/runs
* track global resources and quotas
* spawn `WorkRun`s from `WorkRunRequest`s
* subscribe to `WorkRun` and `TaskRun` events
* grant consumable resources to `WorkRun`s
* track completion to free global resources
* accept user cancellations and route them to the correct runtime object

The `WorkRunner` does not resolve `Work` or `Task` definitions.

### 4.4 WorkRun

A `WorkRun` is a live orchestrator for one workflow execution.

It must:

* own the DAG state
* know which logical steps are pending/running/succeeded/failed/cancelled
* know which `TaskRunRequest` corresponds to each step
* react to granted resources
* decide which ready steps to start
* spawn `TaskRun`s from precomputed `TaskRunRequest`s
* subscribe to `TaskRun` events
* merge step outputs back into workflow state
* enforce retry policy using the precomputed request
* emit `WorkRunEvent`s

A `WorkRun` owns workflow-level execution logic.

### 4.5 TaskRunner

A `TaskRunner` executes tasks using one backend.

Examples:

* shell
* container
* kubernetes
* sql
* http

It must:

* accept a frozen `TaskRunRequest`
* create a `TaskRun`
* start/cancel execution
* expose execution state and events through the `TaskRun`

### 4.6 TaskRun

A `TaskRun` is one concrete task execution attempt.

It must:

* represent one execution of one `TaskRunRequest`
* expose current runtime state
* support start and cancel
* emit execution events
* export a document snapshot

If a retry is needed, a new `TaskRun` is created from the same frozen `TaskRunRequest`.

## 5. Naming and Representation Rules

### 5.1 Runtime Objects vs Documents

Behavioral/runtime objects use plain names:

* `WorkRunner`
* `TaskRunner`
* `WorkRun`
* `TaskRun`

Document/serializable representations use `Doc` suffix:

* `WorkDoc`
* `TaskDoc`
* `TriggerDoc`
* `CronDoc`
* `WorkRunRequestDoc`
* `TaskRunRequestDoc`
* `WorkRunnerDoc`
* `TaskRunnerDoc`
* `WorkRunDoc`
* `TaskRunDoc`

A `*Doc` is the standard document representation of an object. It is used for export, inspection, persistence, transport, and reload.

`*Doc` does not imply catalog persistence.

### 5.2 Document Conversion

Runtime objects expose infallible document export:

```rust
fn as_doc(&self) -> XDoc;
```

This method does not return `Result`.

## 6. Requests vs Runtime Objects

### 6.1 WorkRunRequest

A `WorkRunRequest` is a frozen workflow execution snapshot.

It contains:

* the resolved `workRef`
* trigger information
* resolved workflow inputs
* the logical step graph
* one reference to a frozen `TaskRunRequest` per logical step
* workflow-level policies resolved at trigger time

It does not contain live runtime state.

### 6.2 TaskRunRequest

A `TaskRunRequest` is a frozen task execution snapshot.

It contains:

* `workRef`
* `taskRef`
* `workRunRequestRef`
* `stepName`
* fully resolved task inputs
* fully resolved output destinations
* fully frozen execution backend configuration
* retry/failure/timeout policy if attached at step level

A `TaskRunRequest` is reused across retries. A retry creates a new `TaskRun`, not a new `TaskRunRequest`.

## 7. Why Requests Are Precomputed Up Front

`TaskRunRequest`s are resolved at the same time as the `WorkRunRequest`.

Reason:

* editing or deleting a `Task` after request creation must not change execution behavior
* retries must remain deterministic
* execution must remain reproducible and auditable
* runtime orchestration must not depend on mutable definition state

This means the trigger side, through the `TriggerResolver`, must provide the full frozen request set.

## 8. Start Semantics

Creation and execution start are separate.

### 8.1 Spawn

`spawn()` creates a runtime object.

It does not imply execution starts immediately.

### 8.2 Start

`start()` begins actual execution.

This separation allows:

* registration in the runner registry
* subscription setup before execution begins
* resource tracking before execution begins
* explicit lifecycle transitions

Both `WorkRun` and `TaskRun` have a `start()` method.

## 9. Resource Model

### 9.1 Global Resources

The `WorkRunner` manages global resources.

The initial resource type is:

* `task_permits`

These are consumable permits allowing a `WorkRun` to start task executions.

### 9.2 Granting Resources

The `WorkRunner` grants consumable resources to a `WorkRun`.

The API is:

```rust
fn grant(&self, resources: WorkRunResources) -> Result<WorkRunResources, WorkRunError>;
```

Meaning:

* input = granted resources
* return = unused resources that the `WorkRun` did not consume

This avoids blocking resources that a workflow cannot currently use.

### 9.3 WorkRun Resource Behavior

When granted `task_permits`, the `WorkRun`:

* determines which steps are ready
* starts as many `TaskRun`s as it can within the grant
* returns any unused permits immediately

The `WorkRunner` remains the global authority on resource allocation.
The `WorkRun` remains the local authority on DAG execution.

## 10. Event Model

The system is event-driven.

The `WorkRunner` must not block waiting on individual runs.
It must react to events from many `WorkRun`s and `TaskRun`s concurrently.

### 10.1 WorkRun Events

`WorkRunEvent` must include at least:

* workflow state changes
* step state changes
* task run creation
* task run updates at workflow level
* workflow completion

A `TaskRunCreated` event is mandatory so subscribers can discover and follow the spawned tasks.

Suggested shape:

```rust
pub enum WorkRunEvent {
    StateChanged(WorkRunState),
    StepStateChanged {
        step_name: String,
        state: WorkRunStepState,
    },
    TaskRunCreated {
        step_name: String,
        task_run_ref: String,
    },
    TaskRunUpdated {
        step_name: String,
        task_run_ref: String,
        state: TaskRunState,
    },
    Finished,
}
```

### 10.2 TaskRun Events

`TaskRunEvent` must include at least:

* task state changes
* output updates if useful
* task completion

Suggested shape:

```rust
pub enum TaskRunEvent {
    StateChanged(TaskRunState),
    OutputUpdated {
        output: String,
    },
    Finished,
}
```

### 10.3 Subscription Model

Subscriptions must return a stream/receiver-like handle.

The core API is not callback-based.

Reason:

* caller controls where/how events are consumed
* cleaner ownership and lifetime model
* easier multiplexing in the `WorkRunner`
* better fit for many concurrent runs

The actual implementation may be synchronous or asynchronous.

For this engine, asynchronous streams are preferred because:

* many runs may execute concurrently
* some backends are naturally asynchronous
* the `WorkRunner` must multiplex many sources

## 11. Cron Model

### 11.1 Cron Responsibilities

`Cron` is intentionally small.

It must:

* know when it should fire
* know whether it is allowed to fire now
* emit a `TriggerDoc`

It must not resolve `Work`, `Task`, or artifacts.

### 11.2 Cron Scheduling Strategy

The scheduler must not poll continuously.

It must:

* compute the next occurrence for each active cron
* order them by next occurrence
* sleep until the earliest occurrence or a control event
* wake on:

  * due occurrence
  * new cron
  * modified cron
  * removed cron
  * enabled/disabled cron
  * shutdown

The recommended implementation uses:

* one in-memory map of active crons
* one ordered priority queue by `next_at`
* generation/versioning to invalidate stale queue entries after updates
* one control channel for cron change events

### 11.3 Cron Output

A cron firing produces a `TriggerDoc`.

That trigger is then given to the `TriggerResolver`.

## 12. Data and Resolution Rules

### 12.1 Unified Binding Philosophy

Types describe the resolved value.
Sources describe where the value comes from.

A value may be provided by:

* literal `value`
* artifact `uri`

The type is always the type of the final resolved value.

### 12.2 Unified URI Model

A single URI model is used consistently.

Examples:

* `registry://...`
* `work://inputs?...`
* `work://steps/.../outputs?...`
* `task://inputs?...`
* `task://outputs?...`

The exact resolution grammar belongs in the resolver/runtime implementation. The engine must preserve one consistent mental model.

### 12.3 Request Objects Are Resolved

In `WorkRunRequestDoc` and `TaskRunRequestDoc`, mappings are already resolved.

This means:

* no unresolved `from` remains
* no unresolved `inputMapping` remains
* no unresolved `outputMapping` remains

Runtime objects execute frozen requests, they do not re-resolve definitions.

## 13. Rust Trait Interfaces

The following traits represent the final validated design direction.

### 13.1 WorkRunner

```rust
pub trait WorkRunner: Send + Sync + std::fmt::Debug {
    fn as_doc(&self) -> WorkRunnerDoc;

    fn spawn(
        &self,
        request: WorkRunRequestDoc,
    ) -> Result<Box<dyn WorkRun>, WorkRunnerError>;
}
```

### 13.2 WorkRun

```rust
pub trait WorkRun: Send + Sync + std::fmt::Debug {
    fn as_doc(&self) -> WorkRunDoc;

    fn start(&self) -> Result<(), WorkRunError>;

    fn cancel(&self) -> Result<(), WorkRunError>;

    /// Grants consumable resources to this workflow run.
    /// Returns the unused resources immediately.
    fn grant(
        &self,
        resources: WorkRunResources,
    ) -> Result<WorkRunResources, WorkRunError>;

    fn subscribe(&self) -> WorkRunEventStream;
}
```

### 13.3 TaskRunner

```rust
pub trait TaskRunner: Send + Sync + std::fmt::Debug {
    fn as_doc(&self) -> TaskRunnerDoc;

    fn spawn(
        &self,
        request: TaskRunRequestDoc,
    ) -> Result<Box<dyn TaskRun>, TaskRunnerError>;
}
```

### 13.4 TaskRun

```rust
pub trait TaskRun: Send + Sync + std::fmt::Debug {
    fn as_doc(&self) -> TaskRunDoc;

    fn start(&self) -> Result<(), TaskRunError>;

    fn cancel(&self) -> Result<(), TaskRunError>;

    fn subscribe(&self) -> TaskRunEventStream;
}
```

## 14. Supporting Rust Types

### 14.1 Resource Type

```rust
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WorkRunResources {
    pub task_permits: usize,
}
```

### 14.2 Async Event Streams

```rust
use std::pin::Pin;
use futures_core::Stream;

pub type WorkRunEventStream = Pin<Box<dyn Stream<Item = WorkRunEvent> + Send>>;
pub type TaskRunEventStream = Pin<Box<dyn Stream<Item = TaskRunEvent> + Send>>;
```

Async streams are preferred for the runtime model because the `WorkRunner` must multiplex many concurrent producers.

## 15. Local Registry

The `WorkRunner` local registry stores at least:

* pending `WorkRunRequestDoc`s
* all `TaskRunRequestDoc`s referenced by pending/active workflows
* active `WorkRun`s
* active `TaskRun`s
* resource/accounting information

The local registry is an implementation concern, though it must support the runtime behavior defined here.

## 16. Retry Rules

A retry does not create a new `TaskRunRequestDoc`.

A retry creates:

* a new `TaskRun`
* from the same frozen `TaskRunRequestDoc`

This guarantees deterministic retries.

## 17. Logging

Logs are not embedded as large payloads inside runtime documents.

Recommended model:

* `TaskRun` emits log/state events
* runtime documents may reference log resources
* log storage may be managed separately

The exact log resource type is outside the scope of this core specification.

## 18. Implementation Guidance for Code Generation

### 18.1 Architectural Rules

Copilot or any generator must follow these rules:

* do not modify public trait signatures unless explicitly requested
* do not merge runtime traits with document structs
* do not re-resolve `Work` or `Task` during execution once requests are frozen
* do not make `Cron` resolve `Work`, `Task`, or artifacts
* do not put global resource policy inside `WorkRun`
* do not put DAG logic inside `WorkRunner`
* do not make retries create new `TaskRunRequestDoc`s
* do not use callback-based subscription as the core runtime contract
* do not collapse `spawn()` and `start()` into a single operation

### 18.2 Preferred Internal Structure

A good implementation should contain small units such as:

* resolver
* work runner engine
* task runner implementations
* work run runtime state machine
* task run runtime state machine
* cron scheduler
* local registry abstraction
* event forwarding/multiplexing utilities
* document conversion utilities

### 18.3 Avoid These Anti-Patterns

* handlers or services that both resolve requests and run workflows
* mixing orchestration state and execution backend logic in one large type
* duplicated state machines across runner implementations
* implicit resource ownership with no clear grant/return semantics
* mutable dependence on catalog definitions after request freezing
* hidden background threads/tasks with no event visibility

## 19. Final Ownership Summary

* `Trigger` expresses intent.
* `TriggerResolver` freezes intent into executable requests.
* `Cron` emits triggers only.
* `WorkRunner` supervises global execution and resources.
* `WorkRun` owns workflow orchestration.
* `TaskRunner` executes one backend-specific task.
* `TaskRun` owns one execution attempt.

This is the final validated architecture.
