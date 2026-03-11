//! Ergonomic config accessors

use serde_json::Value;

pub struct ConfigTree(pub Value);

impl ConfigTree {
    /// Get a reference to a value by dot-separated path (e.g. "foo.bar.baz")
    pub fn get(&self, path: &str) -> Option<&Value> {
        let mut current = &self.0;
        for key in path.split('.') {
            match current {
                Value::Object(map) => {
                    current = map.get(key)?;
                }
                _ => return None,
            }
        }
        Some(current)
    }

    /// Get a typed value by path, deserializing to T
    pub fn get_typed<T: serde::de::DeserializeOwned>(&self, path: &str) -> Option<T> {
        self.get(path).and_then(|v| serde_json::from_value(v.clone()).ok())
    }
}
