use crate::document::Document;
use crate::global::Result;

pub trait DocumentParser<T>: Send + Sync {
    fn parse(&self, content: &str) -> Result<Vec<Document<T>>>;
}