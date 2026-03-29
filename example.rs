use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Condvar, Mutex};
use std::thread;
use std::time::Duration;

#[derive(Debug, Clone)]
enum ResponseMode {
    Async,
    Sync,
}

#[derive(Debug, Clone)]
struct Request {
    id: u64,
    component_id: u32,
    payload: String,
    mode: ResponseMode,
}

#[derive(Debug, Clone)]
struct Response {
    request_id: u64,
    component_id: u32,
    payload: String,
}

type SyncSlot = Arc<(Mutex<Option<Response>>, Condvar)>;
type ComponentCallback = Arc<dyn Fn(Response) + Send + Sync + 'static>;

struct Host {
    next_id: AtomicU64,
    input_tx: mpsc::Sender<Request>,
    sync_store: Arc<Mutex<HashMap<u64, SyncSlot>>>,
}

impl Host {
    fn send_async(&self, component_id: u32, payload: impl Into<String>) {
        let req = Request {
            id: self.next_id.fetch_add(1, Ordering::Relaxed),
            component_id,
            payload: payload.into(),
            mode: ResponseMode::Async,
        };

        self.input_tx.send(req).unwrap();
    }

    fn call_sync(&self, component_id: u32, payload: impl Into<String>) -> Response {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);

        let slot: SyncSlot = Arc::new((Mutex::new(None), Condvar::new()));
        self.sync_store.lock().unwrap().insert(id, slot.clone());

        let req = Request {
            id,
            component_id,
            payload: payload.into(),
            mode: ResponseMode::Sync,
        };

        self.input_tx.send(req).unwrap();

        let (lock, cv) = &*slot;
        let mut guard = lock.lock().unwrap();

        while guard.is_none() {
            guard = cv.wait(guard).unwrap();
        }

        let response = guard.take().unwrap();
        self.sync_store.lock().unwrap().remove(&id);
        response
    }
}

fn start_host(callbacks: HashMap<u32, ComponentCallback>) -> Arc<Host> {
    let (input_tx, input_rx) = mpsc::channel::<Request>();
    let (output_tx, output_rx) = mpsc::channel::<Response>();

    let sync_store: Arc<Mutex<HashMap<u64, SyncSlot>>> = Arc::new(Mutex::new(HashMap::new()));

    let host = Arc::new(Host {
        next_id: AtomicU64::new(1),
        input_tx,
        sync_store: sync_store.clone(),
    });

    // Worker thread
    {
        let sync_store = sync_store.clone();
        thread::spawn(move || {
            while let Ok(req) = input_rx.recv() {
                thread::sleep(Duration::from_millis(100));

                let response = Response {
                    request_id: req.id,
                    component_id: req.component_id,
                    payload: format!("processed: {}", req.payload),
                };

                match req.mode {
                    ResponseMode::Async => {
                        output_tx.send(response).unwrap();
                    }
                    ResponseMode::Sync => {
                        if let Some(slot) = sync_store.lock().unwrap().get(&req.id).cloned() {
                            let (lock, cv) = &*slot;
                            *lock.lock().unwrap() = Some(response);
                            cv.notify_one();
                        }
                    }
                }
            }
        });
    }

    // Dispatcher thread
    {
        thread::spawn(move || {
            while let Ok(response) = output_rx.recv() {
                if let Some(callback) = callbacks.get(&response.component_id) {
                    callback(response);
                }
            }
        });
    }

    host
}

fn main() {
    let mut callbacks: HashMap<u32, ComponentCallback> = HashMap::new();

    callbacks.insert(
        1,
        Arc::new(|resp| {
            println!(
                "[component {} async callback] req={} => {}",
                resp.component_id, resp.request_id, resp.payload
            );
        }),
    );

    let host = start_host(callbacks);

    // Component thread
    let host_for_component = host.clone();
    let component_thread = thread::spawn(move || {
        host_for_component.send_async(1, "hello async");

        let response = host_for_component.call_sync(1, "hello sync");
        println!(
            "[component 1 sync wait] req={} => {}",
            response.request_id, response.payload
        );
    });

    component_thread.join().unwrap();

    thread::sleep(Duration::from_millis(300));
}