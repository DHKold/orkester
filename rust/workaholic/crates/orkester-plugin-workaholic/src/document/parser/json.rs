impl DocumentParser for JsonDocumentParser {
    /// Parse a JSON string into a single Document (no multiple documents in JSON).
    fn parse(&self, content: &str) -> Result<Vec<Document>> {
        let document: Document = serde_json::from_str(content)?;
        let documents = vec![document];
        Ok(documents)
    }
}

// === Export the component for use in Orkester ===
pub struct JsonDocumentParserComponent{
    parser: JsonDocumentParser,
}

#[component(
    kind = "workaholic/JsonDocumentParser:1.0",
    name = "JSON Document Parser",
    description = "A simple document parser that converts JSON strings into Document structs.",
)]
impl JsonDocumentParserComponent {
    fn new() -> Self {
        Self { parser: JsonDocumentParser }
    }

    #[handle(ACTION_PARSER_PARSE)]
    fn handle_parse(&self, content: String) -> Result<Vec<Document>> {
        self.parser.parse(&content)
    }
}