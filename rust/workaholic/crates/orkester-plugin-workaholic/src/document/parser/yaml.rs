use serde::Deserialize;

use orkester_plugin::prelude::*;

use workaholic::{DocumentParser, Document, Result};

use super::actions::*;

pub struct YamlDocumentParser;

impl DocumentParser for YamlDocumentParser {
    /// Parse a YAML string into a list of Document structs (YAML supports multiple documents).
    fn parse(&self, content: &str) -> Result<Vec<Document>> {
        let mut documents = Vec::new();
        for deserializer in serde_yaml::Deserializer::from_str(content) {
            let document = Document::deserialize(deserializer)?;
            documents.push(document);
        }
        Ok(documents)
    }
}

// === Export the component for use in Orkester ===
pub struct YamlDocumentParserComponent{
    parser: YamlDocumentParser,
}

#[component(
    kind = "workaholic/YamlDocumentParser:1.0",
    name = "YAML Document Parser",
    description = "A simple document parser that converts YAML strings into Document structs.",
)]
impl YamlDocumentParserComponent {
    fn new() -> Self {
        Self { parser: YamlDocumentParser }
    }

    #[handle(ACTION_PARSER_PARSE)]
    fn handle_parse(&mut self, content: String) -> Result<Vec<Document>> {
        self.parser.parse(&content)
    }
}