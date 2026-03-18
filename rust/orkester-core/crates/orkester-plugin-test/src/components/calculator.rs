//! Calculator component -- binary `f64` arithmetic.
//!
//! Demonstrates rich error handling: division/remainder by zero and unknown
//! operators return structured JSON errors instead of panicking.
//!
//! ## Request / response wire format
//! ```json
//! { "op": "add" | "sub" | "mul" | "div" | "pow" | "rem", "a": <f64>, "b": <f64> }
//! ->  { "result": <f64> }
//! ```

use serde::{Deserialize, Serialize};
use orkester_plugin::sdk::{ComponentHandler, HandlerResponse, Request, ERROR_INVALID_REQUEST};

#[derive(Deserialize)]
struct CalcReq { op: String, a: f64, b: f64 }

#[derive(Serialize)]
struct CalcRes { result: f64 }

/// Stateless calculator handler.
pub struct Calculator;

impl ComponentHandler for Calculator {
    fn handle(&mut self, req: Request) -> HandlerResponse {
        let parsed: CalcReq = match serde_json::from_slice(req.payload()) {
            Ok(v) => v,
            Err(e) => return HandlerResponse::error(ERROR_INVALID_REQUEST, e.to_string()),
        };

        let result: Result<f64, &'static str> = match parsed.op.as_str() {
            "add" => Ok(parsed.a + parsed.b),
            "sub" => Ok(parsed.a - parsed.b),
            "mul" => Ok(parsed.a * parsed.b),
            "div" => if parsed.b == 0.0 { Err("division by zero")    } else { Ok(parsed.a / parsed.b) },
            "pow" => Ok(parsed.a.powf(parsed.b)),
            "rem" => if parsed.b == 0.0 { Err("remainder by zero")   } else { Ok(parsed.a % parsed.b) },
            op    => return HandlerResponse::error(
                ERROR_INVALID_REQUEST,
                format!("unknown op {op:?}; expected add, sub, mul, div, pow, rem"),
            ),
        };

        match result {
            Ok(value) => HandlerResponse::json(&CalcRes { result: value }),
            Err(msg)  => HandlerResponse::error(ERROR_INVALID_REQUEST, msg),
        }
    }
}
