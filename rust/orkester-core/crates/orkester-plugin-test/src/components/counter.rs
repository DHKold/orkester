//! Counter component -- a stateful `i64` counter.
//!
//! Demonstrates mutable per-component state through a plain Rust struct field.
//!
//! ## Request / response wire format
//! ```json
//! { "type": "Inc" | "Dec" | "Get" | "Reset" }  ->  { "value": <i64> }
//! ```

use serde::{Deserialize, Serialize};
use orkester_plugin::sdk::{require_json, ComponentHandler, HandlerResponse, Request, ERROR_INVALID_REQUEST};

#[derive(Deserialize)]
struct CounterReq { #[serde(rename = "type")] op: String }

#[derive(Serialize)]
struct CounterRes { value: i64 }

pub struct Counter {
    value: i64,
}

impl Counter {
    pub fn new() -> Self { Self { value: 0 } }
}

impl ComponentHandler for Counter {
    fn handle(&mut self, req: Request) -> HandlerResponse {
        let parsed: CounterReq = match require_json(&req) {
            Ok(v)  => v,
            Err(e) => return e,
        };
        match parsed.op.as_str() {
            "Inc"   => { self.value += 1; HandlerResponse::json(&CounterRes { value: self.value }) }
            "Dec"   => { self.value -= 1; HandlerResponse::json(&CounterRes { value: self.value }) }
            "Get"   => HandlerResponse::json(&CounterRes { value: self.value }),
            "Reset" => { self.value = 0;  HandlerResponse::json(&CounterRes { value: self.value }) }
            other   => HandlerResponse::error(
                ERROR_INVALID_REQUEST,
                format!("unknown counter op {other:?}; expected Inc, Dec, Get, Reset"),
            ),
        }
    }
}
