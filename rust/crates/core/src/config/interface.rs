//! Config interface and loader trait

use serde_json::Value;
use std::error::Error;

pub trait ConfigLoader {
    fn load(path: &str) -> Result<Value, Box<dyn Error>>;
}
