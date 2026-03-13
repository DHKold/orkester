//! HTTP REST gateway — dynamically-routed Axum server.
//!
//! External HTTP requests are forwarded to the appropriate handler server via
//! the hub messaging bus.  Responses are held in a pending map keyed by a
//! correlation id until the matching hub reply arrives.
//!
//! # Module layout
//!
//! | Module     | Responsibility                                               |
//! |------------|--------------------------------------------------------------|
//! | `state`    | `AppState`, `RouteKey`, `RouteRegistration`, path matching.  |
//! | `handlers` | Axum request handlers (`list_routes`, `dynamic_route`).      |
//! | `hub`      | Hub-message processing task and route registration.          |
//! | `server`   | `AxumRestServer` lifecycle, Tokio runtime, Axum wiring.      |

pub(super) mod handlers;
pub(super) mod hub;
pub(super) mod server;
pub(super) mod state;

pub use server::{AxumRestServer, AxumRestServerBuilder};
