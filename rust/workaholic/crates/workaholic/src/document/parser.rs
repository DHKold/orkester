use crate::document::Document;
use crate::global::Result;

pub trait DocumentParser: Send + Sync {
    fn parse(&self, content: &str) -> Result<Vec<Document>>;
}