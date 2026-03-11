//! TOML config loader

use super::interface::ConfigLoader;
use serde_json::Value;
use std::error::Error;
use std::fs;

pub struct TomlConfigLoader;

impl ConfigLoader for TomlConfigLoader {
    fn load(path: &str) -> Result<Value, Box<dyn Error>> {
        let data = fs::read_to_string(path)?;
        let value: toml::Value = toml::from_str(&data)?;
        let json = serde_json::to_value(value)?;
        Ok(json)
    }
}
