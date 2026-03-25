//! Worker — drives a Workflow through its full lifecycle, step by step.
//!
//! # Architecture
//!
//! Execution is split across four layers:
//!
//! | Module    | Responsibility                                              |
//! |-----------|-------------------------------------------------------------|
//! | `traits`  | Public `Worker` trait.                                      |
//! | `local`   | `LocalWorker` — lifecycle phases, delegates to DAG.         |
//! | `dag`     | Kahn's topological sort; wave-parallel execution; policies. |
//! | `step`    | Per-step retry loop, timeout, and executor dispatch.        |
//!
//! # Task resolution
//!
//! All [`Task`] definitions required by a [`Work`] are fetched from the
//! Workspace server in a **single round-trip** before the first wave executes.
//! Individual steps receive a pre-resolved `Task` so `step` has no direct
//! dependency on the workspace client.
//!
//! [`Task`]: orkester_common::domain::Task
//! [`Work`]: orkester_common::domain::Work

pub(super) mod dag;
pub(super) mod local;
pub(super) mod step;
pub(super) mod traits;

pub use local::LocalWorker;
pub use traits::Worker;
