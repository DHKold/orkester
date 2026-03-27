use std::collections::HashMap;

use orkester_plugin::prelude::*;
use workaholic::{Document, DocumentLoader, DocumentParser, Result, WorkaholicError};

use super::actions::*;

// Temporary solution, will be replaced by a more flexible plugin-based parser registry in the future.
use crate::document::parser::json::JsonDocumentParser;
use crate::document::parser::yaml::YamlDocumentParser;

// ─── LocalFsLoader ─────────────────────────────────────────────────────────────

/// Loads documents from the local filesystem.
/// Delegates parsing to configurable [`DocumentParser`] trait objects keyed by file extension.
pub struct LocalFsLoader {
    paths: Vec<String>,
    recursive: bool,
    extensions: HashMap<String, Box<dyn DocumentParser>>,
}

impl LocalFsLoader {
    pub fn new(
        paths: Vec<String>,
        recursive: bool,
        extensions: HashMap<String, Box<dyn DocumentParser>>,
    ) -> Self {
        Self { paths, recursive, extensions }
    }

    fn load_from_path(&self, path: &str) -> Result<Vec<Document>> {
        let metadata = std::fs::metadata(path).map_err(|e| WorkaholicError::Other(format!("Cannot stat '{}': {}", path, e)))?;
        if metadata.is_file() {
            self.load_from_file(path)
        } else if metadata.is_dir() {
            self.load_from_directory(path)
        } else {
            Err(WorkaholicError::Other(format!("'{}' is neither a file nor a directory", path)))
        }
    }

    fn load_from_file(&self, file_path: &str) -> Result<Vec<Document>> {
        let supported = self.extensions.keys().cloned().collect::<Vec<_>>().join(", ");
        let ext = std::path::Path::new(file_path)
            .extension()
            .and_then(|s| s.to_str())
            .ok_or_else(|| {
                WorkaholicError::InvalidDocument(format!(
                    "'{}' has no file extension; supported: {}", file_path, supported
                ))
            })?
            .to_string();
        if !self.extensions.contains_key(&ext) {
            return Err(WorkaholicError::InvalidDocument(format!(
                "Unsupported extension '{}' for '{}'; supported: {}",
                ext, file_path, supported
            )));
        }
        let content = std::fs::read_to_string(file_path)
            .map_err(|e| WorkaholicError::Other(format!("Failed to read '{}': {}", file_path, e)))?;
        self.parse_content(&content, &ext)
    }

    fn load_from_directory(&self, dir_path: &str) -> Result<Vec<Document>> {
        let mut documents = Vec::new();
        let entries = std::fs::read_dir(dir_path).map_err(|e| WorkaholicError::Io(e))?;
        for entry in entries {
            let entry = entry.map_err(|e| WorkaholicError::Io(e))?;
            let path_buf = entry.path();
            let path = path_buf
                .to_str()
                .ok_or_else(|| WorkaholicError::Other("Invalid path in directory".to_string()))?;
            documents.extend(self.load_from_path(path)?);
        }
        Ok(documents)
    }

    fn parse_content(&self, content: &str, ext: &str) -> Result<Vec<Document>> {
        let parser = self.extensions.get(ext).ok_or_else(|| WorkaholicError::InvalidDocument(format!("No parser for extension '{}'", ext)))?;
        parser.parse(content)
    }
}

impl DocumentLoader for LocalFsLoader {
    fn load(&self) -> Result<Vec<Document>> {
        let mut documents = Vec::new();
        for path in &self.paths {
            documents.extend(self.load_from_path(path)?);
        }
        Ok(documents)
    }
}

// ─── Configuration ─────────────────────────────────────────────────────────────

#[derive(serde::Deserialize)]
pub struct LocalFsLoaderConfig {
    /// List of file or directory paths to load documents from.
    pub paths: Vec<String>,
    /// Whether to scan directories recursively.
    #[serde(default)]
    pub recursive: bool,
    /// Maps file extension (e.g. `"yaml"`) to parser kind
    /// (e.g. `"workaholic/YamlDocumentParser:1.0"`).
    #[serde(default)]
    pub extensions: HashMap<String, String>,
}

// ─── Component ─────────────────────────────────────────────────────────────────

pub struct LocalFsLoaderComponent {
    loader: LocalFsLoader,
}

#[component(
    kind = "workaholic/LocalFsLoader:1.0",
    name = "Local Filesystem Loader",
    description = "Loader that reads documents from the local filesystem based on specified paths and parameters.",
)]
impl LocalFsLoaderComponent {
    pub fn new(config: LocalFsLoaderConfig) -> Self {
        let mut parsers: HashMap<String, Box<dyn DocumentParser>> = HashMap::new();
        for (ext, kind) in &config.extensions {
            let parser: Box<dyn DocumentParser> = match kind.as_str() {
                "workaholic/YamlDocumentParser:1.0" | "yaml" | "yml" => Box::new(YamlDocumentParser),
                "workaholic/JsonDocumentParser:1.0" | "json" => Box::new(JsonDocumentParser),
                other => {
                    log::warn!("Unknown parser kind '{}' for extension '{}' — skipping", other, ext);
                    continue;
                }
            };
            parsers.insert(ext.clone(), parser);
        }
        let loader = LocalFsLoader::new(config.paths, config.recursive, parsers);
        Self { loader }
    }

    #[handle(ACTION_LOAD_DOCUMENTS)]
    fn handle_load(&mut self, _: ()) -> workaholic::Result<Vec<Document>> {
        self.loader.load()
    }
}