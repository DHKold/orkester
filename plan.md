# Orkester + Workaholic — Development Plan

_Current date: 2026-03-31_

Derived from `todo.md` with additional items identified from the current codebase state.
Items tagged `[UNCLEAR]` need refinement before implementation begins.

---

## How to read this document

**Priority tiers** (P1–P5)
- P1 — blocking / critical path
- P2 — high value / should ship soon
- P3 — important architecture / non-urgent
- P4 — product expansion
- P5 — long-horizon / nice-to-have

**Status tags**
- `[Done]` — merged + shipping
- `[Partial]` — code exists but incomplete / untested end-to-end
- `[Todo]` — not yet started
- `[UNCLEAR]` — requires design discussion before implementation

---

## 1. Immediate — Next Iteration

These are the items already in motion and should close in the next sprint.

### 1.1 S3 Document Loader `[P1]` `[Partial]`

The implementation exists (`document/loader/s3/`) including `scanner.rs`,
`watcher.rs`, `client.rs`, `auth.rs`. The loader can do an initial scan and
start watchers. Missing:

- [ ] Integration/E2E test against a real S3 bucket or localstack
- [ ] Verify the watcher re-scan cycle works end-to-end (events propagated to catalog)

### 1.2 Auto-Import of Crons `[P1]` `[Todo]`

The `CronScheduler` exists and works. The S3Loader and LocalFsLoader can load
`CronDoc`s. Missing link: after a loader scan delivers a `CronDoc` to the
catalog, it should automatically register the cron with the scheduler.

- [ ] Define the wiring point (WorkflowServer start / catalog publish action?)
- [ ] Implement auto-register on cron doc arrival
- [ ] Implement auto-unregister when a cron doc is removed from source

### 1.3 Test a Real DBT Work `[P2]` `[Todo]`

- [ ] Define a `dbt run` task template using `ContainerTaskRunner` / `KubernetesTaskRunner`

Note: The DBT project itself is out of scope for Orkester — it should be developed and owned by the Data Platform team. It is provided as a docker image that the task runner can execute.

### 1.4 UI Bug Fixes `[P2]` `[Todo]`

- [ ] Metrics page: stop polling once loaded (currently re-subscribes on every render)
- [ ] WorkRun list: update to use the new schema:
      - [ ] In spec: workRef, trigger.type & trigger.at
      - [ ] In status: state
- [ ] Cron list: show next fire time (currently missing)

### 1.5 External: Workaholic YAML Tooling for DP `[P1]` `[Todo]`

- [ ] Provide a tool or SDK for the Data Platform team to produce Workaholic YAML (Works, Tasks, Crons)
- [ ] Will be done as a branch of the DP Scripts repo (not in Orkester core)

---

## 2. Architecture — Foundation Pieces

### 2.1 Global Document / Resource Model `[P1]` `[Partial]`

Currently `kind` strings are used inconsistently across the codebase, sometime as 'id', sometime as a type. Should be the kind document represented by the spec/status.
The naming convention is undocumented, there is no schema for the document structure, and the resolution rules for versioning are not defined.
This makes it hard to add new document types and leads to confusion about how to reference them.

- [ ] Write an ADR defining what `kind`, `name`, `version` mean
      - Convention: `<domain>/<Type>:<version>` (e.g. `workaholic/Task:1.0`)
      - SEMVER: version is a semver string; matching rules for resolution (exact match, range match, etc) TBD
      - Naming convention for standard kinds
- [ ] Produce a registry (TBD) of all standard kinds in use today
- [ ] Refactor all kind strings in the codebase to match
- [ ] Produce a JSON Schema for each kind and documentation for how to use it

### 2.2 Document Persistors `[P2]` `[Partial]`

`MemoryPersistor` and `LocalFsPersistor` exist. `DocumentPersistor` trait is
in the `workaholic` crate. Missing:

- [ ] Ensure `LocalFsDocumentPersistor` component is wired + tested end-to-end
- [ ] Use dynamic persistor in the catalog/workflow/metrics (currently hardcoded to memory)

### 2.3 Catalog Refactor `[P3]` `[Todo]`

The current catalog is a flat in-memory map. Long-horizon improvements:

- [ ] Grouping of Tasks and Works (namespace scoping is partial, group-level needed)
- [ ] Historization of catalog changes (append-only log)
- [ ] Versioning — store multiple `version` values per `(namespace, name)` and resolve by semver range
- [ ] Resource lifecycle states (draft / active / deprecated / deleted)
- [ ] Handle dependencies between resources (e.g. if a Task depends on a Document, the catalog should prevent deletion of the Document without first removing the Task)
- [ ] Schema validation on document creation/update
- [ ] Query API (filter by kind, namespace, tags, state)
- [ ] Import/export (YAML / JSON roundtrip, including bulk)
- [ ] Diffing between versions

### 2.4 Logging SDK + Server `[P2]` `[Todo]`

- [ ] Make the Logging configuration global (not per-plugin)
- [ ] Fallback queue in the SDK: buffer log events when the host is unreachable)
- [ ] Config-driven filtering: by level, by component name, by time range, etc.

### 2.5 Authentication SDK `[P3]` `[Todo]`

- [ ] Add `Authenticator` trait to `orkester-plugin` crate
- [ ] `NoAuthenticator` — allows all requests (for dev/test)
- [ ] `PwdAuthenticator` — validates username/password against a config
- [ ] `JwtAuthenticator` — validates Bearer tokens
- [ ] `OidcAuthenticator` — fetches JWKS, validates claims
- [ ] `KeycloakAuthenticator` — specialized OIDC for Keycloak with realm support
- [ ] Macro helpers to secure methods with `#[secured]` attribute

### 2.6 Authorization SDK `[P3]` `[Todo]`

- [ ] Add `Authorizer` trait to `orkester-plugin` crate
- [ ] `StaticAuthorizer` — rule table in config
- [ ] `RbacAuthorizer` — role/permission model
- [ ] `OpaAuthorizer` — delegate to OPA (plugin `w/OPA` already exists as a stub)
- [ ] Update the macro helpers to also check authorization (e.g. `#[secured(...)]`)

---

## 3. Plugin: `w/CORE` (`orkester-plugin-workaholic`) `[P2]`

### 3.1 HttpTaskRunner `[Partial]`

Already implemented (`http.rs`). Missing:

- [ ] Authentication header support (Bearer, Basic)
- [ ] Retry on transient HTTP errors
- [ ] `[UNCLEAR]` Should the runner support non-JSON response bodies? Yes

### 3.2 LocalFsArtifactRegistry `[Todo]`

- [ ] Define `ArtifactRegistry` trait if not yet present
- [ ] Implement a local-filesystem-backed registry (suitable for dev/test)

Note: A registry is a kind of organized (filesystem-like) storage for 'Artifacts', which can be files (or folders), simple values or structured data.
The registry is accessible via a URI scheme, providing a way to reference artifacts (e.g. `<registry>://path/to/artifact?fields`). Note that `<registry>` identifies a registry instance, it does not refer to its kind which is opaque.
All registries expose a common interface for storing and retrieving artifacts, but the underlying implementation can vary (e.g. local filesystem, S3, database, etc).
A registry can be global, domain-scoped, time-scoped, etc depending on the use case (e.g. a workRun-scoped registry for intermediate artifacts, a global registry for shared artifacts, etc).

### 3.3 ConfigMapDocumentLoader `[P3]` `[Todo]`

- [ ] Load Kubernetes `ConfigMap` resources as Workaholic documents
- [ ] Requires the embedded kube client (now available via `orkester-plugin-workaholic`)
- [ ] Use kubernetes API to watch for changes and update the catalog in real-time
- [ ] Maybe relocate to a dedicated `w/kubernetes` plugin if it grows beyond just config maps / runner ? (Check the size of the kube client dependency and whether it makes sense to isolate it)

---

## 4. Plugin: `w/AWS` (`orkester-plugin-workaholic-aws`) `[P2]`

The crate exists but is a placeholder (`lib.rs` is a single comment line).

### 4.1 S3DocumentLoader `[P1]` `[Todo for AWS crate]`

The S3 loader lives in `orkester-plugin-workaholic` today (not the AWS crate).

- [ ] Refactor it into the AWS plugin crate since it has an AWS-specific dependency (the S3 client)
- [ ] Add support for multiple buckets / prefixes (currently hardcoded to one)
- [ ] Add support for versioning (e.g. load the latest version of a document based on S3 object versions or timestamps)

### 4.2 S3DocumentPersistor `[P2]` `[Todo]`

- [ ] Implement `DocumentPersistor` backed by S3 (put/get/delete/list via prefix)
- [ ] Use the same SigV4 auth client already in `document/loader/s3/auth.rs`

### 4.3 S3ArtifactRegistry `[P3]` `[Todo]`

- [ ] Implement `ArtifactRegistry` backed by S3 (put/get/delete/list with defined key structure)
- [ ] Store artifacts in S3 with a defined key structure (e.g. `s3://bucket/prefix/path/to/artifact?fields`)

### 4.4 S3LogSink `[P3]` `[Todo]`

- [ ] Implement a `LogSink` that writes logs to S3 objects
- [ ] Should support log rotation (e.g. by size or time) to avoid unbounded growth of log objects
- [ ] Consider the latency implications of writing logs to S3 (eventual consistency, write performance) and whether it is suitable for real-time log streaming or more for archival purposes
- [ ] Should the log sink write individual log events as they come, or batch them into larger objects? -> Make it configurable, but default to batching for performance

### 4.5 EcsTaskRunner / Ec2TaskRunner `[P4]` `[Todo]`

- [ ] `EcsTaskRunner` — submit ECS tasks via the AWS SDK
- [ ] Fargate vs. EC2 launch type? Config-driven.

### 4.6 CloudwatchLogSink `[P3]` `[Todo]`

- [ ] Implement a `LogSink` that writes logs to CloudWatch Logs
- [ ] Write log events to CloudWatch Logs via `PutLogEvents`

### 4.7 LambdaTaskRunner `[P4]` `[Todo]`

- [ ] Implement a `TaskRunner` that invokes AWS Lambda functions
- [ ] Invoke a Lambda function with task inputs; poll for async result

### 4.8 EcrArtifactRegistry `[P4]` `[Todo]`

- [ ] Implement an `ArtifactRegistry` that stores container image references in ECR
- [ ] Store image references with a defined key structure (e.g. `ecr://repository/path/to/image:tag`)
- [ ] Or should we rather provide a generic `OciArtifactRegistry` that can work with any OCI registry?

### 4.9 DynamoDbDocumentLoader / Persistor `[P4]` `[Todo]`

- [ ] Implement a `DocumentLoader` that loads documents from a DynamoDB table
- [ ] Implement a `DocumentPersistor` that stores documents in a DynamoDB table
- [ ] Use DynamoDB as a document store (suitable for serverless deployments)

---

## 5. Plugin: `w/SQL` (`orkester-plugin-workaholic-sql`) `[P5]`

Crate exists. All items are `[Todo]`.

- [ ] `SqlTaskRunner` — execute a SQL statement or stored procedure as a task
- [ ] `SqlDocumentLoader` — load documents from a SQL table
- [ ] `SqlDocumentPersistor` — store entities in SQL
- [ ] `SqlArtifactRegistry` — store artifact references in SQL
- [ ] `SqlLogSink` — write log records to SQL
- [ ] `SqlAuthorizer` — policy rules stored in SQL (duplicate in todo.md — keep one)
- [ ] Support for multiple SQL dialects (Postgres, MySQL, etc) — either via a common subset or via dialect-specific implementations

---

## 6. Plugin: `w/Kafka` `[P5]` `[Todo]`

No crate exists yet. Create `orkester-plugin-workaholic-kafka`.

- [ ] `KafkaTaskRunner` — publish a message and wait for a reply topic
- [ ] `KafkaDocumentLoader` — consume a topic as a document stream
- [ ] `KafkaLogSink` — publish log events to Kafka
- [ ] `[UNCLEAR]` Consumer group management — how does the loader commit offsets? (Configurable: auto-commit vs manual commit after processing each batch)
- [ ] Allow flexible topic configuration (brokers, topic name, serialization format, etc) via config
- [ ] Allow using an Avro schema registry for message serialization (optional, config-driven)

---

## 7. Plugin: `data-platform` `[P4]` `[Todo]`

No crate exists yet.

- [ ] Define `Product`, `Subscription`, `Maintenance`, `Tenant` document kinds
- [ ] `ProductBuilder` trait — maps a product spec into executable workflow artifacts
- [ ] `WorkaholicProductBuilder` — produces Works + Tasks + Crons
- [ ] `AirflowProductBuilder` — produces Airflow DAGs + Operators + Schedules
- [ ] `ArgoProductBuilder` — produces Argo Workflows + CronWorkflows
- [ ] UI: product list view
- [ ] UI: product builder / configurator view
- [ ] `[UNCLEAR]` Relationship between a Product version and the catalog version of its constituent Tasks/Works — immutable snapshot or live reference? -> Snapshot for stability, but with a clear versioning strategy to manage updates.

---

## 8. REST Server Improvements `[P2]`

- [ ] Use proper libraries for HTTP serving (e.g. `axum`) instead of ad-hoc hyper server
- [ ] Allow exposing the OpenAPI spec via an endpoint (e.g. `/openapi.json`)
- [ ] Modularize handlers (separate handler module per resource group)
- [ ] Serve the UI as a proper SPA (handle client-side routing, correct `Content-Type`, cache headers)
- [ ] Flexible routing: multi-target dispatch, per-route fallback
- [ ] SSL/TLS termination (or document that it should be delegated to the ingress)
- [ ] CORS + custom header support

---

## 9. Workflow Server / WorkRunner Improvements `[P2]`

### 9.1 Input/Output Handling

- [ ] Use registries for inputs/outputs.
- [ ] Use a scoped registry (like `workrun://steps/X/outputs?key`)
- [ ] Validate inputs against declared types at resolve time (not just at runtime)
- [ ] Support artifact-reference outputs (not just inline `Value`)
- [ ] Output mapping from `work://steps/X/outputs?key` should fail loudly when X hasn't run
- [ ] Resolution of artifacts should be done by a dedicated `ArtifactResolver`.

### 9.2 Retry Logic `[Todo]`

- [ ] Defined at the step level (not task level) since it is a workflow-level concern
- [ ] Configurable retry policy (e.g. max attempts, backoff strategy, etc)
- [ ] The WorkRunner should handle the retry logic. Each try creates a new TaskRun from the same TaskRunRequest (with the `attempt` number incremented).
- [ ] The TaskRunner should be idempotent or able to handle duplicate TaskRunRequests for the same step (e.g. by using the `attempt` number to distinguish retries).
- [ ] The WorkRun status should reflect the retry attempts (e.g. `state: retrying`, `attempts: N`, etc) and surface the failure reason of the last attempt.
- [ ] For the future:
      - [ ] Support for failing paths (e.g. if max attempts exceeded, mark the step as failed and optionally trigger a compensating workflow or alert)
      - [ ] Support for conditional retries based on error type (e.g. only retry on transient errors, not on validation errors) — would require error classification in the TaskRunner and propagation of error types to the WorkRunner
- [ ] UI:
      - [ ] Show retry attempts and failure reasons in the WorkRun detail view
      - [ ] Allow triggering a manual retry from the UI with input overrides (e.g. to fix a failed step and retry with corrected inputs)
      - [ ] Show the retry policy configuration in the step details (e.g. max attempts, backoff strategy) for visibility
- [ ] Consider how to visualize long-running retries in the UI (e.g. if there is a long backoff, show that the step is waiting for retry rather than just failed)

### 9.3 Timeouts & Cancellation `[Todo]`

- [ ] Per-task timeout enforced by `TaskRunner` (already partially in `KubernetesTaskRunner` and `HttpTaskRunner`)
- [ ] Per-workflow timeout enforced by `WorkRun` / `WorkRunner`
- [ ] `[UNCLEAR]` What happens to running tasks when a workflow timeout fires — hard cancel or grace period? -> Implement a grace period: when the workflow timeout is reached, mark the WorkRun as `timed_out` and send a cancellation signal to all running tasks. The tasks can then choose to handle the cancellation gracefully (e.g. clean up resources, save intermediate state) before exiting. If a task does not exit within a certain grace period after receiving the cancellation signal, it can be forcefully killed by the runner.
- [ ] Cancellation API: allow cancelling a running WorkRun from the API/UI, which triggers the same cancellation logic as a timeout (mark as cancelled, send cancellation signal to tasks)
- [ ] UI: show timeout and cancellation status in the WorkRun detail view, including which tasks were running at the time of timeout/cancellation and their final status (e.g. cancelled, completed, failed)

### 9.4 Error Reporting `[Todo]`

- [ ] Ensure all errors in the TaskRunner are propagated back to the WorkRunner with sufficient context (error type, message, stack trace if available)
- [ ] The WorkRun status should reflect the error state of failed steps (e.g. `state: failed`, `error: {type, message, ...}`)
- [ ] UI: show error details in the WorkRun detail view for failed steps, with the ability to expand for more context (e.g. stack trace, logs around the error time)
- [ ] Consider categorizing errors into types (e.g. validation error, execution error, timeout error) to allow for better handling in the UI and potential retry logic (e.g. only retry on execution errors, not on validation errors)

### 9.5 Automatic Triggering `[Todo]`

- [ ] Could be implemented using a dedicated `WorkaholicTaskRunner` which fire Works. That way a Work can trigger another Work by calling the API from a task step. This allows for flexible triggering logic (e.g. trigger based on time, events, or as a follow-up step in another workflow).

---

## 10. Kubernetes Task Runner — Follow-ups `[P2]`

The embedded kube client rewrite is done. Known gaps:

- [ ] Cancel: currently marks state cancelled and sends event, but does not actually delete the Job in the cluster (the delete happens only after normal completion). Fix by running `delete_job` on cancel path.
- [ ] Job naming: currently uses only the first UUID segment (`orkester-{x}`) — verify uniqueness is sufficient or use full UUID with prefix truncation to stay under 63 chars.
- [ ] Labels: add `orkester.io/work-run`, `orkester.io/task-ref` labels to the Job manifest for observability.
- [ ] Resource limits: expose `resources.requests/limits` as config keys (CPU, memory).
- [ ] Image pull secrets: expose `imagePullSecrets` as a config key.
- [ ] `[UNCLEAR]` TTL-after-finished: should the Job set `ttlSecondsAfterFinished` instead of explicit deletion in the runner? (Configurable: if TTL is set, rely on Kubernetes to clean up finished Jobs; if not set, the runner will delete the Job after completion.)

---

## 11. Better UI `[P2]`

Beyond the immediate bug fixes in §1.4:

- [ ] WorkRun detail page: show per-step timeline, inputs, outputs, logs
- [ ] Task catalog browser: searchable, filterable by kind/namespace/tags
- [ ] Cron list: show next fire time, last fire result
- [ ] Trigger a Work manually from the UI with input overrides
- [ ] MUST USE Vanilla CSS + JS (no frameworks) for performance and simplicity, but we can use libraries for specific components (e.g. a date picker, a code editor, etc) as long as they don't require a full framework
- [ ] Globaly: consistent styling, responsive layout, clear error states, loading states, etc

---
