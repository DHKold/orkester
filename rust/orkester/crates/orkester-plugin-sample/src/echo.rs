//! Simple message echo with an optional prefix string.

use orkester_plugin::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct EchoConfig {
    /// String prepended to every echoed message. Defaults to empty.
    #[serde(default)]
    pub prefix: String,
}

#[derive(Debug, Deserialize)]
pub struct EchoRequest {
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct EchoResponse {
    pub message: String,
}

pub struct EchoComponent {
    prefix: String,
}

impl EchoComponent {
    pub fn new(config: EchoConfig) -> Self {
        Self { prefix: config.prefix }
    }
}

#[component(
    kind        = "sample/Echo:1.0",
    name        = "Echo",
    description = "Echoes incoming messages back to the caller, optionally with a prefix."
)]
impl EchoComponent {
    #[handle("sample/Echo")]
    fn echo(&mut self, req: EchoRequest) -> Result<EchoResponse> {
        Ok(EchoResponse { message: format!("{}{}", self.prefix, req.message) })
    }
}
