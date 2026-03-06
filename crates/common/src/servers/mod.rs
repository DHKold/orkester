pub mod metrics;
pub mod rest;
pub mod state;
pub mod workflow;

pub struct ServerContext<Tx, Rx> {
    /// Receiver to read server messages
    pub receiver: Option<std::sync::mpsc::Receiver<Rx>>,
    /// Sender to send messages to the server
    pub sender: Option<std::sync::mpsc::Sender<Tx>>,
    /// Handle to the server thread, which can be joined on shutdown
    pub handle: std::thread::JoinHandle<()>,
}
