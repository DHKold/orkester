use std::{
    ptr,
    sync::{Mutex, OnceLock},
    sync::atomic::{AtomicBool, AtomicPtr, Ordering},
};

use crate::abi::{AbiHost, AbiRequest};
use super::{buffer::LogBuffer, record::LogRecord};

// ─── Per-plugin globals (one copy per cdylib) ─────────────────────────────────

static LOG_HOST:  AtomicPtr<AbiHost>         = AtomicPtr::new(ptr::null_mut());
static FALLBACK:  AtomicBool                  = AtomicBool::new(false);
static BUFFER:    OnceLock<Mutex<LogBuffer>>  = OnceLock::new();
static PLUGIN_ID: OnceLock<String>            = OnceLock::new();

// ─── Public API ───────────────────────────────────────────────────────────────

/// Connect SDK logging to the host.  Called once per plugin from the generated
/// entry-point (`export_plugin_root*!` macros) before user code runs.
pub fn init_logging(host: *mut AbiHost, identity: &str) {
    if host.is_null() { return; }
    PLUGIN_ID.get_or_init(|| identity.to_owned());
    LOG_HOST.store(host, Ordering::Release);
    flush_buffer();
}

/// Return the plugin-identity string (package name set at entry-point time).
pub fn plugin_id() -> &'static str {
    PLUGIN_ID.get().map(String::as_str).unwrap_or("unknown")
}

/// Route `record` through the appropriate path (host, buffer, or fallback).
pub fn send_log(record: LogRecord) {
    let host = LOG_HOST.load(Ordering::Acquire);
    if !host.is_null() {
        deliver(host, &record);
    } else if FALLBACK.load(Ordering::Acquire) {
        fallback_print(&record);
    } else {
        buffer_push(record);
    }
}

// ─── Private helpers ──────────────────────────────────────────────────────────

fn buffer_push(record: LogRecord) {
    let buf = BUFFER.get_or_init(|| Mutex::new(LogBuffer::default()));
    if let Ok(mut lock) = buf.lock() {
        lock.push(record);
    }
}

fn flush_buffer() {
    let Some(buf) = BUFFER.get() else { return };
    let records: Vec<LogRecord> = match buf.lock() {
        Ok(mut lock) => lock.drain().collect(),
        Err(_) => return,
    };
    let host = LOG_HOST.load(Ordering::Acquire);
    if host.is_null() { return; }
    for record in records {
        deliver(host, &record);
    }
}

fn deliver(host: *mut AbiHost, record: &LogRecord) {
    let Ok(payload) = serde_json::to_vec(record) else {
        set_fallback();
        return;
    };
    let fmt = b"log/json+fire";
    let req = AbiRequest {
        id: 0,
        format:      fmt.as_ptr(),
        format_len:  fmt.len() as u32,
        payload:     payload.as_ptr(),
        payload_len: payload.len() as u32,
    };
    let res = unsafe { ((*host).handle)(host, req) };
    unsafe { ((*host).free_response)(host, res) };
}

fn set_fallback() {
    FALLBACK.store(true, Ordering::Release);
}

fn fallback_print(record: &LogRecord) {
    eprintln!("[{}] {} {} - {}", record.level, record.plugin_id, record.target, record.message);
}
