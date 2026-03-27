
use thiserror::Error;

use crate::document::Document;
use crate::global::Result;

#[derive(Debug, Error)]
pub enum LoaderError {
    #[error("Wrong file extension for path {0}: expected one of {1}")]
    WrongFileExtension(String, String),
    #[error("Invalid path {0}: {1}")]
    WrongPath(String, String),
    #[error("Failed to read file {0}: {1}")]
    ReadError(String, String),
}

pub trait DocumentLoader: Send + Sync {
    fn load(&self) -> Result<Vec<Document>>;
}
