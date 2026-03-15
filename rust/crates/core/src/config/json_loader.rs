//! JSON config loader

use super::interface::ConfigLoader;
use serde_json::Value;
use std::error::Error;
use std::fs;

pub struct JsonConfigLoader;

impl ConfigLoader for JsonConfigLoader {
    fn load(path: &str) -> Result<Value, Box<dyn Error>> {
        let data = fs::read_to_string(path)?;
        let value: Value = serde_json::from_str(&data)?;
        Ok(value)
    }
}
