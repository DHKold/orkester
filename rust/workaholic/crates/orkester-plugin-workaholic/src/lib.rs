//! Workaholic workflow execution engine — Orkester plugin.
//!
//! This crate contains:
//! - `workflow` — runtime traits, events, and implementations for `WorkRunner`,
//!   `WorkRun`, `TaskRunner`, and `TaskRun`.
//! - `catalog` — the catalog server component.
//! - `document` — document loaders, parsers, and persisters.

pub mod catalog;
pub mod document;
pub mod workflow;

pub use workflow::{
    ContainerTaskRunner, HttpTaskRunner, ShellTaskRunner, ThreadWorkRunner,
    TaskRun, TaskRunError, TaskRunEvent, TaskRunEventStream, TaskRunner, TaskRunnerError,
    WorkRun, WorkRunError, WorkRunEvent, WorkRunEventStream, WorkRunResources,
    WorkRunner, WorkRunnerError,
};