//! JSON config loader

use serde_json::Value;
use std::fs;
use std::error::Error;
use super::interface::ConfigLoader;

pub struct JsonConfigLoader;

impl ConfigLoader for JsonConfigLoader {
    fn load(path: &str) -> Result<Value, Box<dyn Error>> {
        let data = fs::read_to_string(path)?;
        let value: Value = serde_json::from_str(&data)?;
        Ok(value)
    }
}
