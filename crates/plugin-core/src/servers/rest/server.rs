//! `AxumRestServer` and its builder.
//!
//! `start()` delegates each concern to a focused private helper so the
//! top-level method reads as a plain narrative of the boot sequence:
//!
//! ```text
//! bind_address  →  bridge_hub_channel  →  build_router  →  serve
//! ```

use std::sync::{Arc, Mutex};

use axum::{routing::get, Router};
use orkester_common::messaging::Message;
use orkester_common::plugin::servers::{Server, ServerBuilder, ServerContext, ServerError};
use orkester_common::log_info;
use serde_json::Value;

use super::handlers::{dynamic_route_handler, list_routes_handler, openapi_handler};
use super::hub::hub_message_task;
use super::state::AppState;

// ── AxumRestServer ────────────────────────────────────────────────────────────

pub struct AxumRestServer {
    config: Value,
    /// Oneshot sender used by `stop()` to trigger graceful shutdown.
    shutdown_tx: Mutex<Option<tokio::sync::oneshot::Sender<()>>>,
    /// Handle to the server thread so `stop()` can join it.
    thread_handle: Mutex<Option<std::thread::JoinHandle<()>>>,
}

impl Server for AxumRestServer {
    fn start(&self, ctx: ServerContext) -> Result<(), ServerError> {
        let channel = ctx.channel;
        let bind_addr = self.bind_address();

        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
        *self.shutdown_tx.lock().unwrap() = Some(shutdown_tx);

        let handle = std::thread::spawn(move || {
            Self::build_runtime().block_on(async move {
                let state = Arc::new(AppState::new(channel.to_hub));
                let hub_rx = Self::bridge_hub_channel(channel.from_hub);
                tokio::spawn(hub_message_task(hub_rx, state.clone()));
                let router = Self::build_router(state);
                Self::serve(router, &bind_addr, shutdown_rx).await;
            });
        });

        *self.thread_handle.lock().unwrap() = Some(handle);
        Ok(())
    }

    fn stop(&self) -> Result<(), ServerError> {
        if let Some(tx) = self.shutdown_tx.lock().unwrap().take() {
            let _ = tx.send(());
        }
        if let Some(handle) = self.thread_handle.lock().unwrap().take() {
            handle.join().ok();
        }
        Ok(())
    }
}

impl AxumRestServer {
    fn bind_address(&self) -> String {
        self.config
            .get("bind")
            .and_then(|v| v.as_str())
            .unwrap_or("0.0.0.0:8080")
            .to_string()
    }

    fn build_runtime() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("failed to build Tokio runtime")
    }

    /// Spawn a thread that forwards messages from the synchronous `from_hub`
    /// receiver into a Tokio mpsc channel so the async `hub_message_task` can
    /// await them without blocking the runtime.
    fn bridge_hub_channel(
        from_hub: std::sync::mpsc::Receiver<Message>,
    ) -> tokio::sync::mpsc::UnboundedReceiver<Message> {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<Message>();
        std::thread::spawn(move || {
            while let Ok(msg) = from_hub.recv() {
                if tx.send(msg).is_err() {
                    break;
                }
            }
        });
        rx
    }

    fn build_router(state: Arc<AppState>) -> Router {
        Router::new()
            .route("/v1/openapi.json", get(openapi_handler))
            .route("/v1/routes", get(list_routes_handler))
            .fallback(dynamic_route_handler)
            .with_state(state)
    }

    async fn serve(
        router: Router,
        bind_addr: &str,
        shutdown_rx: tokio::sync::oneshot::Receiver<()>,
    ) {
        let listener = tokio::net::TcpListener::bind(bind_addr)
            .await
            .expect("bind failed");

        log_info!("Listening on {}.", bind_addr);

        axum::serve(listener, router)
            .with_graceful_shutdown(async move {
                let _ = shutdown_rx.await;
                log_info!("Shutdown signal received.");
            })
            .await
            .expect("server error");

        log_info!("Server stopped.");
    }
}

// ── Builder ───────────────────────────────────────────────────────────────────

pub struct AxumRestServerBuilder;

impl ServerBuilder for AxumRestServerBuilder {
    fn build(&self, config: Value) -> Result<Box<dyn Server>, ServerError> {
        Ok(Box::new(AxumRestServer {
            config,
            shutdown_tx: Mutex::new(None),
            thread_handle: Mutex::new(None),
        }))
    }
}
