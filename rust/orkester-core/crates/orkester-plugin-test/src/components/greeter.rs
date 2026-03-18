//! Greeter component -- personalised multi-language greeting.
//!
//! Demonstrates:
//! - Optional JSON fields (`language` defaults to `"en"`).
//! - Plugin -> host callbacks: every greeting fires a `Log` event to the host.
//!
//! Supported languages: `en` (default), `fr`, `es`, `de`, `ja`.
//!
//! ## Request / response wire format
//! ```json
//! { "name": "Alice" }                   ->  { "greeting": "Hello, Alice!",    "language": "en" }
//! { "name": "Alice", "language": "fr" }  ->  { "greeting": "Bonjour, Alice !", "language": "fr" }
//! ```

use serde::{Deserialize, Serialize};
use orkester_plugin::{abi, sdk::{require_json, ComponentHandler, HandlerResponse, Request, FLAG_UTF8, MSG_TYPE_JSON}};

#[derive(Deserialize)]
struct GreetReq {
    name: String,
    #[serde(default)]
    language: Option<String>,
}

#[derive(Serialize)]
struct GreetRes { greeting: String, language: String }

pub struct Greeter {
    /// Non-owning pointer to the host for log callbacks.
    host: *mut abi::Host,
}

// SAFETY: the host pointer is valid for the plugin's lifetime.
unsafe impl Send for Greeter {}
unsafe impl Sync for Greeter {}

impl Greeter {
    pub fn new(host: *mut abi::Host) -> Self { Self { host } }
}

impl ComponentHandler for Greeter {
    fn handle(&mut self, req: Request) -> HandlerResponse {
        let parsed: GreetReq = match require_json(&req) {
            Ok(v)  => v,
            Err(e) => return e,
        };

        let lang = parsed.language.as_deref().unwrap_or("en");
        let greeting = match lang {
            "fr" => format!("Bonjour, {} !", parsed.name),
            "es" => format!("Hola, {}!", parsed.name),
            "de" => format!("Hallo, {}!", parsed.name),
            "ja" => format!("Konnichiwa, {}!", parsed.name),
            _    => format!("Hello, {}!", parsed.name),
        };

        // Plugin -> host callback.
        log_to_host(self.host, req.id(), &format!("greeted '{}' (lang={lang})", parsed.name));

        HandlerResponse::json(&GreetRes { greeting, language: lang.to_owned() })
    }
}

/// Fire-and-forget JSON log message to the host.
fn log_to_host(host: *mut abi::Host, id: u64, message: &str) {
    if host.is_null() { return; }
    #[derive(Serialize)]
    struct Log<'a> { #[serde(rename = "type")] t: &'static str, message: &'a str }
    let Ok(payload) = serde_json::to_vec(&Log { t: "Log", message }) else { return };
    let req = abi::Request {
        id, format: MSG_TYPE_JSON, flags: FLAG_UTF8,
        payload: payload.as_ptr(), len: payload.len() as u32,
    };
    unsafe {
        let res = ((*host).handle)(host, req);
        ((*host).free_response)(host, res);
    }
}
