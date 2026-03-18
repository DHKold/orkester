//! Echo component -- reflects any payload back to the caller unchanged.
//!
//! The simplest possible component: no state, no dispatch logic.
//! Format and flags are preserved so the response is byte-for-byte identical
//! to whatever the host sent.

use orkester_plugin::sdk::{ComponentHandler, HandlerResponse, Request};

/// Stateless echo handler.
pub struct Echo;

impl ComponentHandler for Echo {
    fn handle(&mut self, req: Request) -> HandlerResponse {
        // Mirror the payload back with the same format and flags.
        HandlerResponse {
            format: req.format(),
            flags:  req.flags(),
            payload: req.payload().to_vec(),
        }
    }
}
