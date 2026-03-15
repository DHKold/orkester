//! Metrics server — exposes runtime counters at `GET /v1/metrics`.
//!
//! Plugins interact with this server exclusively through hub messages:
//!
//! - `update_metric` — update a named counter (see [`server`] for the schema).
//! - `http_request`  — forwarded by the REST server; responded to automatically.
//!
//! # Module layout
//!
//! | Module      | Responsibility                                                    |
//! |-------------|-------------------------------------------------------------------|
//! | `collector` | Generic `f64` metric store; internal to this module.              |
//! | `rest_api`  | Route registration message and HTTP response builder.             |
//! | `server`    | Lifecycle, event loop, and hub message dispatch.                  |

pub(super) mod collector;
pub(super) mod rest_api;
pub(super) mod rest_handler;
pub(super) mod server;

pub use server::{MetricsServer, MetricsServerBuilder};
