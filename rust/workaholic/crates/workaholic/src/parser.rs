use crate::{error::Result, traits::DocumentParser};

/// Parses documents from YAML content.
pub struct YamlDocumentParser;

impl DocumentParser for YamlDocumentParser {
    fn parse<T>(&self, content: &str) -> Result<T>
    where
        T: for<'de> serde::Deserialize<'de>,
    {
        serde_yaml::from_str(content).map_err(crate::WorkaholicError::Yaml)
    }
}

/// Parses documents from JSON content.
pub struct JsonDocumentParser;

impl DocumentParser for JsonDocumentParser {
    fn parse<T>(&self, content: &str) -> Result<T>
    where
        T: for<'de> serde::Deserialize<'de>,
    {
        serde_json::from_str(content).map_err(crate::WorkaholicError::Json)
    }
}
