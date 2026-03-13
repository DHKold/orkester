//! Axum request handlers.
//!
//! `dynamic_route_handler` is the hot path for every non-intrinsic request.
//! It is kept intentionally short by delegating to two private helpers:
//!
//! 1. `send_hub_request` — allocate a correlation id, register a pending waiter,
//!    build the `http_request` message, and send it to the hub.
//! 2. `await_hub_response` — poll the one-shot channel with a 30-second deadline
//!    and convert the result into an HTTP response.

use std::sync::Arc;
use std::time::Duration;

use axum::{
    body::Bytes,
    extract::State,
    http::{Method, StatusCode, Uri},
    response::{IntoResponse, Response},
    Json,
};
use orkester_common::messaging::Message;
use orkester_common::{log_debug, log_trace};
use serde_json::{json, Value};

use super::state::{AppState, RouteRegistration};

// ── GET /v1/openapi.json ─────────────────────────────────────────────────────

pub(super) async fn openapi_handler(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    Json(state.build_openapi_spec())
}

// ── GET /v1/routes ────────────────────────────────────────────────────────────

pub(super) async fn list_routes_handler(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let routes = state.routes.read().unwrap();
    let list: Vec<Value> = routes
        .iter()
        .map(|(k, v)| json!({ "method": k.method, "path": k.path, "registrant": v.target }))
        .collect();
    log_trace!("REST GET /v1/routes — listing {} route(s)", list.len());
    Json(json!({ "routes": list }))
}

// ── Fallback dynamic handler ──────────────────────────────────────────────────

pub(super) async fn dynamic_route_handler(
    State(state): State<Arc<AppState>>,
    method: Method,
    uri: Uri,
    body: Bytes,
) -> Response {
    let method_str = method.to_string();
    let path = uri.path().to_string();
    log_debug!("REST {} {} received", method_str, path);

    let reg = match state.resolve_route(&method_str, &path) {
        Some(r) => r,
        None => {
            log_debug!("REST {} {} → 404 (no registered route)", method_str, path);
            // Don't emit metrics for unrecognised paths — they may be probes.
            state.send_metric("rest.requests_total", "increment", 1.0);
            state.send_metric("rest.responses_4xx", "increment", 1.0);
            return (StatusCode::NOT_FOUND, Json(json!({ "error": "route not found" })))
                .into_response();
        }
    };

    // Skip instrumentation for metrics-server routes to avoid self-observation noise.
    let instrument = !state.is_metrics_target(&reg.target);

    if instrument {
        state.send_metric("rest.requests_total", "increment", 1.0);
        state.active_request_start();
    }

    let (corr_id, rx) =
        match send_hub_request(&state, &reg, &method_str, &path, parse_body(&body)) {
            Ok(pair) => pair,
            Err(response) => {
                if instrument { state.active_request_end(); }
                return response;
            }
        };

    let response = await_hub_response(&state, rx, &method_str, &path, corr_id, instrument).await;
    if instrument { state.active_request_end(); }
    response
}

// ── Private helpers ───────────────────────────────────────────────────────────

fn parse_body(body: &Bytes) -> Value {
    if body.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(body).unwrap_or(Value::Null)
    }
}

/// Allocate a correlation id, register a pending waiter, and send an
/// `http_request` message to the hub.
///
/// Returns `Ok((corr_id, receiver))` on success, or `Err(Response)` when the
/// hub channel is already closed.
fn send_hub_request(
    state: &Arc<AppState>,
    reg: &RouteRegistration,
    method: &str,
    path: &str,
    body: Value,
) -> Result<(u64, tokio::sync::oneshot::Receiver<Message>), Response> {
    let corr_id = state.next_correlation_id();
    let (tx, rx) = tokio::sync::oneshot::channel::<Message>();
    state.register_pending(corr_id, tx);

    log_trace!(
        "REST dispatching to '{}' (method={} path={} correlation_id={})",
        reg.target, method, path, corr_id,
    );

    let msg = Message::new(
        corr_id,
        "", // hub stamps source
        reg.target.as_str(),
        "http_request",
        json!({
            "correlation_id": corr_id,
            "method": method,
            "path": path,
            "body": body,
        }),
    );

    if !state.send_to_hub(msg) {
        state.remove_pending(corr_id);
        log_debug!(
            "REST {} {} → hub disconnected (correlation_id={})",
            method, path, corr_id,
        );
        state.send_metric("rest.hub_disconnected", "increment", 1.0);
        state.send_metric("rest.responses_5xx", "increment", 1.0);
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({ "error": "hub disconnected" })),
        )
            .into_response());
    }

    Ok((corr_id, rx))
}

/// Wait up to 30 s for the hub response on `rx` and convert it to a [`Response`].
async fn await_hub_response(
    state: &Arc<AppState>,
    rx: tokio::sync::oneshot::Receiver<Message>,
    method: &str,
    path: &str,
    corr_id: u64,
    instrument: bool,
) -> Response {
    match tokio::time::timeout(Duration::from_secs(30), rx).await {
        Ok(Ok(reply)) => {
            let status = reply
                .content
                .get("status")
                .and_then(|v| v.as_u64())
                .and_then(|s| StatusCode::from_u16(s as u16).ok())
                .unwrap_or(StatusCode::OK);
            let body = reply.content.get("body").cloned().unwrap_or(Value::Null);
            log_trace!(
                "REST {} {} ← {} from '{}' (correlation_id={}, body={} bytes)",
                method, path, status.as_u16(), reply.source, corr_id,
                body.to_string().len(),
            );
            log_debug!("REST {} {} → {}", method, path, status.as_u16());
            let status_code = status.as_u16();
            if instrument {
                if status_code < 400 {
                    state.send_metric("rest.responses_2xx", "increment", 1.0);
                } else if status_code < 500 {
                    state.send_metric("rest.responses_4xx", "increment", 1.0);
                } else {
                    state.send_metric("rest.responses_5xx", "increment", 1.0);
                }
            }
            (status, Json(body)).into_response()
        }
        Ok(Err(_)) => {
            log_debug!(
                "REST {} {} → upstream disconnected (correlation_id={})",
                method, path, corr_id,
            );
            if instrument {
                state.send_metric("rest.hub_disconnected", "increment", 1.0);
                state.send_metric("rest.responses_5xx", "increment", 1.0);
            }
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({ "error": "upstream disconnected" })),
            )
                .into_response()
        }
        Err(_) => {
            state.remove_pending(corr_id);
            log_debug!(
                "REST {} {} → timeout (correlation_id={})",
                method, path, corr_id,
            );
            if instrument {
                state.send_metric("rest.hub_timeouts", "increment", 1.0);
                state.send_metric("rest.responses_5xx", "increment", 1.0);
            }
            (
                StatusCode::GATEWAY_TIMEOUT,
                Json(json!({ "error": "upstream timeout" })),
            )
                .into_response()
        }
    }
}
