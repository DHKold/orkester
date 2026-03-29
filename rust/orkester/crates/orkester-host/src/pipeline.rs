//! Staged async pipeline for `AbiHost.handle` (component → host direction).
//!
//! # Architecture
//!
//! ```text
//! component thread
//!   └─ ABI ingress handler (enqueue only — no heavy work here)
//!            │
//!        input queue
//!            │
//!       worker thread  ←── user-supplied routing closure
//!            │
//!     ┌──────┴──────────────────────────────────┐
//!     │ Sync                                     │ Async
//!     │ write to sync_store[id], notify CV       │ push to output queue
//!     │ ingress thread wakes, returns response   │        │
//!     └──────────────────────────────────────────┘  dispatcher thread
//!                                                         │
//!                                                   async callbacks
//! ```
//!
//! The ABI ingress handler captures only lightweight primitives (an `AtomicU64`
//! counter, a channel sender, and a mutex-guarded slot map) and does no routing
//! logic itself.  All routing is isolated to the worker thread.
//!
//! # Extending
//! - Multiple workers: replace `spawn_worker` with a pool; the channel is N:1.
//! - Per-component output queues: replace the single `output_rx` with a map.
//! - Backpressure: switch to `crossbeam_channel::bounded`.
//! - Timeouts / cancellation: add a deadline field to `HostRequest`.
//! - Monitoring: inspect queue depths and add metrics to `PipelineIngress`.

use std::collections::HashMap;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    mpsc, Arc, Condvar, Mutex,
};

use orkester_plugin::abi::{AbiRequest, AbiResponse};
use orkester_plugin::sdk::Host;

// ─── Public types ──────────────────────────────────────────────────────────────

/// Delivery contract for a request crossing the ABI boundary.
///
/// Detected from the `format` suffix sent by the caller:
///
/// | Suffix     | Mode    | Caller blocks? | Response delivered? |
/// |------------|---------|----------------|---------------------|
/// | *(none)*   | `Sync`  | Yes            | To caller (condvar) |
/// | `+async`   | `Async` | No (empty ack) | To async callback   |
/// | `+fire`    | `Fire`  | No (empty ack) | Never               |
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResponseMode {
    /// The component thread blocks on a condvar until the worker produces a result.
    Sync,
    /// The component thread receives an empty ack; the result goes to the async callback.
    Async,
    /// The component thread receives an empty ack; no response is ever produced.
    /// Use this for one-way events (e.g. document change notifications).
    Fire,
}

/// An inbound request dequeued by the worker thread.
///
/// Carries the fully extracted payload (no raw pointers) and the routing metadata
/// needed to return a result to the right waiter or dispatcher.
#[derive(Debug)]
pub struct HostRequest {
    /// Monotonically increasing id assigned by the ingress handler.
    pub id:      u64,
    /// Decoded `format` string from the original `AbiRequest`.
    pub format:  String,
    /// Decoded payload bytes from the original `AbiRequest`.
    pub payload: Vec<u8>,
    /// How to deliver the response once the worker is done.
    pub mode:    ResponseMode,
}

/// A result produced by the worker, ready to be delivered.
#[derive(Debug, Clone)]
pub struct HostResponse {
    /// Matches `HostRequest::id` for correlation.
    pub request_id: u64,
    /// JSON-encoded response payload.
    pub payload:    Vec<u8>,
}

/// Called by the dispatcher thread for each processed async response.
pub type AsyncCallback = Arc<dyn Fn(HostResponse) + Send + Sync + 'static>;

// ─── Internal types ────────────────────────────────────────────────────────────

/// One-shot synchronization slot shared between the ingress handler and the worker.
///
/// The ingress thread parks on the condvar; the worker writes the response and
/// signals it.
type SyncSlot = Arc<(Mutex<Option<HostResponse>>, Condvar)>;

/// Outstanding sync requests, keyed by request id.
type SyncStore = Arc<Mutex<HashMap<u64, SyncSlot>>>;

// ─── Ingress state ─────────────────────────────────────────────────────────────

/// All state required by the ABI ingress handler.
///
/// Fields must be `Send + Sync` because `Host::with_callback` requires its
/// closure to satisfy both bounds.  `Mutex<Sender>` makes the `mpsc::Sender`
/// shareable across the potential re-entrancy of the callback.
struct PipelineIngress {
    next_id:    AtomicU64,
    input_tx:   Mutex<mpsc::Sender<HostRequest>>,
    sync_store: SyncStore,
}

impl PipelineIngress {
    fn new(input_tx: mpsc::Sender<HostRequest>, sync_store: SyncStore) -> Arc<Self> {
        Arc::new(Self {
            next_id:   AtomicU64::new(1),
            input_tx:  Mutex::new(input_tx),
            sync_store,
        })
    }

    /// Entry point called for every ABI request.  Does the minimum needed:
    /// assigns an id, extracts bytes, enqueues, and resolves via the chosen mode.
    fn handle(&self, req: AbiRequest) -> AbiResponse {
        let (format, payload) = extract_abi_fields(&req);
        let mode = detect_mode(&format);
        let id   = self.next_id.fetch_add(1, Ordering::Relaxed);

        match mode {
            ResponseMode::Sync  => self.handle_sync(req.id, format, payload, id),
            ResponseMode::Async => self.handle_async(req.id, format, payload, id),
            ResponseMode::Fire  => self.handle_fire(req.id, format, payload, id),
        }
    }

    /// Registers a sync slot, enqueues the request, blocks until the worker
    /// writes a result, then returns the result as an `AbiResponse`.
    fn handle_sync(
        &self,
        abi_id:  u64,
        format:  String,
        payload: Vec<u8>,
        id:      u64,
    ) -> AbiResponse {
        let slot: SyncSlot = Arc::new((Mutex::new(None), Condvar::new()));
        self.sync_store.lock().unwrap().insert(id, slot.clone());

        if self.enqueue(id, format, payload, ResponseMode::Sync).is_err() {
            self.sync_store.lock().unwrap().remove(&id);
            return empty_abi_response(abi_id);
        }

        // Block until the worker notifies us.
        let (lock, cv) = &*slot;
        let mut guard  = lock.lock().unwrap();
        while guard.is_none() {
            guard = cv.wait(guard).unwrap();
        }

        self.sync_store.lock().unwrap().remove(&id);
        into_abi_response(guard.take().unwrap())
    }

    /// Enqueues the request and returns an empty ack immediately.
    fn handle_async(
        &self,
        abi_id:  u64,
        format:  String,
        payload: Vec<u8>,
        id:      u64,
    ) -> AbiResponse {
        let _ = self.enqueue(id, format, payload, ResponseMode::Async);
        empty_abi_response(abi_id)
    }

    /// Enqueues the request and returns an empty ack.  The worker processes
    /// the request (e.g. dispatches an event) but produces no response.
    fn handle_fire(
        &self,
        abi_id:  u64,
        format:  String,
        payload: Vec<u8>,
        id:      u64,
    ) -> AbiResponse {
        let _ = self.enqueue(id, format, payload, ResponseMode::Fire);
        empty_abi_response(abi_id)
    }

    fn enqueue(
        &self,
        id:      u64,
        format:  String,
        payload: Vec<u8>,
        mode:    ResponseMode,
    ) -> Result<(), mpsc::SendError<HostRequest>> {
        let req = HostRequest { id, format, payload, mode };
        self.input_tx.lock().unwrap().send(req)
    }
}

// ─── Thread helpers ────────────────────────────────────────────────────────────

/// Spawns the worker thread.
///
/// The worker calls `handler` for every dequeued `HostRequest` and routes the
/// result via [`route_response`].  The thread exits when the input channel is
/// closed (i.e. the `PipelineIngress` is dropped).
fn spawn_worker<H>(
    input_rx:   mpsc::Receiver<HostRequest>,
    output_tx:  mpsc::Sender<HostResponse>,
    sync_store: SyncStore,
    handler:    H,
) where
    H: Fn(HostRequest) -> Option<HostResponse> + Send + 'static,
{
    std::thread::Builder::new()
        .name("host-pipeline-worker".into())
        .spawn(move || {
            while let Ok(req) = input_rx.recv() {
                let mode = req.mode.clone();
                let id   = req.id;
                let resp = handler(req);
                route_response(mode, id, resp, &sync_store, &output_tx);
            }
            log::debug!("[pipeline/worker] input channel closed — thread exiting");
        })
        .expect("failed to spawn host-pipeline-worker");
}

/// Routes a worker result to the correct destination based on `mode`.
fn route_response(
    mode:       ResponseMode,
    id:         u64,
    resp:       Option<HostResponse>,
    sync_store: &SyncStore,
    output_tx:  &mpsc::Sender<HostResponse>,
) {
    match (mode, resp) {
        (ResponseMode::Sync, Some(r)) => resolve_sync(sync_store, id, r),
        // Even when the worker returns None, the waiting ingress must be unblocked.
        (ResponseMode::Sync, None) => {
            resolve_sync(sync_store, id, HostResponse { request_id: id, payload: Vec::new() });
        }
        (ResponseMode::Async, Some(r)) => { let _ = output_tx.send(r); }
        (ResponseMode::Async, None)    => {}
        // Fire-and-forget: the worker still processes/dispatches the request,
        // but no response is ever sent back to the caller.
        (ResponseMode::Fire, _) => {}
    }
}

/// Writes a response to the sync slot for `id` and wakes the waiting ingress thread.
fn resolve_sync(store: &SyncStore, id: u64, response: HostResponse) {
    if let Some(slot) = store.lock().unwrap().get(&id).cloned() {
        let (lock, cv) = &*slot;
        *lock.lock().unwrap() = Some(response);
        cv.notify_one();
    }
}

/// Spawns the dispatcher thread.
///
/// The dispatcher delivers each async response to all registered callbacks.
/// A slow callback delays delivery only for that response; the worker is never
/// blocked by callback speed.
fn spawn_dispatcher(
    output_rx: mpsc::Receiver<HostResponse>,
    callbacks: HashMap<u32, AsyncCallback>,
) {
    std::thread::Builder::new()
        .name("host-pipeline-dispatcher".into())
        .spawn(move || {
            while let Ok(resp) = output_rx.recv() {
                for cb in callbacks.values() {
                    cb(resp.clone());
                }
            }
            log::debug!("[pipeline/dispatcher] output channel closed — thread exiting");
        })
        .expect("failed to spawn host-pipeline-dispatcher");
}

// ─── ABI helpers ───────────────────────────────────────────────────────────────

/// Safely copies `format` and `payload` out of the raw ABI request.
fn extract_abi_fields(req: &AbiRequest) -> (String, Vec<u8>) {
    let format = if req.format.is_null() || req.format_len == 0 {
        String::new()
    } else {
        let bytes = unsafe { std::slice::from_raw_parts(req.format, req.format_len as usize) };
        std::str::from_utf8(bytes).unwrap_or("").to_string()
    };

    let payload = if req.payload.is_null() || req.payload_len == 0 {
        Vec::new()
    } else {
        unsafe { std::slice::from_raw_parts(req.payload, req.payload_len as usize) }.to_vec()
    };

    (format, payload)
}

/// Converts a `HostResponse` into a heap-allocated `AbiResponse`.
///
/// The payload is allocated via `Box::into_raw` so that
/// `callback_host_free_response` (installed by `Host::with_callback`) can free
/// it with `Box::from_raw`.
fn into_abi_response(resp: HostResponse) -> AbiResponse {
    static FMT: &str = "std/json";
    let len = resp.payload.len() as u32;
    let ptr = Box::into_raw(resp.payload.into_boxed_slice()) as *mut u8;
    AbiResponse {
        id:          resp.request_id,
        format:      FMT.as_ptr(),
        format_len:  FMT.len() as u32,
        payload:     ptr,
        payload_len: len,
    }
}

/// Returns an empty `AbiResponse` (used for async acks or error indicators).
fn empty_abi_response(id: u64) -> AbiResponse {
    AbiResponse {
        id,
        format:      std::ptr::null(),
        format_len:  0,
        payload:     std::ptr::null_mut(),
        payload_len: 0,
    }
}

// ─── Mode detection ────────────────────────────────────────────────────────────

/// Determines `ResponseMode` from the suffix appended to the ABI `format` field.
///
/// | Suffix   | Mode    |
/// |----------|---------|
/// | *(none)* | `Sync`  |
/// | `+async` | `Async` |
/// | `+fire`  | `Fire`  |
fn detect_mode(format: &str) -> ResponseMode {
    if format.ends_with("+fire") {
        ResponseMode::Fire
    } else if format.ends_with("+async") {
        ResponseMode::Async
    } else {
        ResponseMode::Sync
    }
}

// ─── Public constructor ────────────────────────────────────────────────────────

/// Builds a routing `Host` backed by the ingress → worker → dispatch pipeline.
///
/// # Arguments
///
/// - `handler` — receives each [`HostRequest`] and returns an optional
///   [`HostResponse`].  Returning `None` is valid (e.g. for side-effect-only
///   requests): sync callers still unblock with an empty payload.
/// - `callbacks` — async-response targets keyed by an arbitrary `u32` component
///   id.  Pass [`HashMap::new()`] when async delivery is not yet needed.
///
/// # Threading
///
/// This function spawns two threads (`host-pipeline-worker` and
/// `host-pipeline-dispatcher`) before returning.  Both exit naturally when the
/// returned `Host` is dropped (channels close, `recv()` unblocks).
pub fn make_pipeline_host<H>(
    handler:   H,
    callbacks: HashMap<u32, AsyncCallback>,
) -> Host
where
    H: Fn(HostRequest) -> Option<HostResponse> + Send + 'static,
{
    let (input_tx,  input_rx)  = mpsc::channel::<HostRequest>();
    let (output_tx, output_rx) = mpsc::channel::<HostResponse>();
    let sync_store = Arc::new(Mutex::new(HashMap::<u64, SyncSlot>::new()));

    spawn_worker(input_rx, output_tx, sync_store.clone(), handler);
    spawn_dispatcher(output_rx, callbacks);

    let ingress = PipelineIngress::new(input_tx, sync_store);

    Host::with_callback(move |req: AbiRequest| ingress.handle(req))
}
