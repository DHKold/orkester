use serde_json::Value;

use orkester_plugin::sdk::Host;

use workaholic::{
    document::{Document, DocumentsLoader, DocumentsLoaderError, DocumentParser},
    utils::default_false,
}

/// Configuration for the LocalFsDocumentsLoader component.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalFsDocumentsLoaderConfig {
    // Path to a directory or a single file to load documents from.
    pub path: String,
    // Recursively search subdirectories if `path` is a directory. Defaults to false.
    #[serde(default="default_false")]
    pub recursive: Option<bool>,
    // Mapping of file extensions to parser kinds, e.g. {"yaml": "yaml_parser", "json": "json_parser"}
    pub parsers: HashMap<String, String>,
}

/// A plugin component that implements the DocumentsLoader trait to load documents from the local filesystem.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalFsDocumentsLoader {
    config: LocalFsDocumentsLoaderConfig,
    supported_extensions: Vec<String>,
    host: Host,
}

/// The handle for the load() method of LocalFsDocumentsLoader.
#[component(
    kind = "workaholic/LocalFsDocumentsLoader:1.0",
    name = "Local Filesystem Documents Loader",
    description = "Loads documents from a local directory tree or a single file.",
)]
impl LocalFsDocumentsLoader {
    /// Constructor for LocalFsDocumentsLoader.
    fn new(host: Host, config: LocalFsDocumentsLoaderConfig) -> Self {
        let supported_extensions = config.parsers.keys().cloned().collect();
        LocalFsDocumentsLoader { config, supported_extensions, host }
    }

    /// Load documents from the configured path. This method is secured with access control based on the path.
    #[handle(DOCUMENTS_LOAD_HANDLE)]
    #[secured(action = "...")]
    fn load(&self) -> Result<Vec<Document<Value>>, Error> {
        let path = self.config.path.as_str();
        let recursive = self.config.recursive.unwrap_or(false);

        // Find all files under `path` (recursively if `recursive` is true) with .yaml/.yml/.json extensions, and load them as documents.
        let files = self.find_files(path, recursive)?;

        // Load each file as documents and collect results.
        let mut documents = Vec::new();
        for file in files {
            match self.load_file(&file) {
                Ok(docs) => documents.extend(docs),
                Err(e) => return Err(DocumentsLoaderError::FileLoadError(file.clone(), e.to_string()).into()),
            }
        }
        Ok(documents)
    }

    /// Find files under the given path with the specified filter. This is a helper method for `load()`.
    #[secured(action = "...")]
    fn find_files(&self, path: &str, recursive: bool) -> Result<Vec<String>, Error> {
        // Validate the input path and check if it exists. Return an error if the path is invalid or does not exist.
        let as_path = std::path::Path::new(path);
        if path.is_empty() {
            return Err(DocumentsLoaderError::WrongPath(path.to_string(), "Path cannot be empty".to_string()));
        }

        if !as_path.exists() {
            return Err(DocumentsLoaderError::WrongPath(path.to_string(), "Path does not exist".to_string()));
        }

        // If it's a file, return it as the only result
        if as_path.is_file() {
            return Ok(vec![path.to_string()]);
        }

        // If it's a directory, walk through it (recursively if `recursive` is true) and collect all files with .yaml/.yml/.json extensions.
        if as_path.is_dir() {
            let mut files = Vec::new();
            let walker = if recursive {
                walkdir::WalkDir::new(as_path).follow_links(true)
            } else {
                walkdir::WalkDir::new(as_path).max_depth(1).follow_links(true)
            };

            for entry in walker.into_iter().filter_map(|e| e.ok()) {
                if entry.file_type().is_file() {
                    let ext = entry.path().extension().and_then(|e| e.to_str()).unwrap_or_default();
                    if self.supported_extensions.contains(&ext.to_string()) {
                        files.push(entry.path().to_string_lossy().to_string());
                    }
                }
            }
            return Ok(files);
        }

        Err(DocumentsLoaderError::WrongPath(path.to_string(), "Path is neither a file nor a directory".to_string()))
    }

    #[secured(action = "...")]
    fn load_file(&self, path: &str) -> Result<Vec<Document<Value>>, Error> {
        // Determine the file extension and look up the corresponding parser in `self.config.parsers`. Return an error if the extension is not supported.
        let ext: &str = std::path::Path::new(path).extension().and_then(|e| e.to_str()).unwrap_or_default();
        let parser_kind: &str = self.config.parsers.get(ext).ok_or_else(|| {
            DocumentsLoaderError::WrongFileExtension(path.to_string(), self.supported_extensions.join(", "))
        })?;
        let parser: Box<dyn DocumentParser<Value>> = self.get_parser(parser_kind)?;

        // Call the appropriate parser based on the file extension. For example, if the extension is "yaml", call the parser specified in `self.config.parsers["yaml"]`.
        let content: String = std::fs::read_to_string(path).map_err(|e| {
            DocumentsLoaderError::ReadError(path.to_string(), e.to_string())
        })?;
        let parsed_docs: Vec<Document<Value>> = parser.parse(&content)?;
        
        // Check access control for each loaded document.
        for doc in &parsed_docs {
            self.check_document_access(&doc.kind, &doc.name, &doc.version)?;
        }
        
        Ok(parsed_docs)
    }

    #[secured(action = "...")]
    fn check_document_access(&self, kind: &str, name: &str, version: &str) -> Result<(), Error> {
        // Access control is automatically handled by the #[secured] attribute, so this function can be empty or contain additional logic if needed.
        Ok(())
    }

    // TODO: provide a macro to generate host calls with proper error handling and logging, to avoid boilerplate in methods like `get_parser()`.
    // Example usage:
    // ```
    // #[enveloppe("orkester/CreateComponent")]
    // fn create_component(&self, request: ) -> Result<SomeComponent, Error> { ... }
    // ```
    fn get_parser(&self, parser_kind: &str) -> Result<Box<dyn DocumentParser<Value>>, Error> {
        let identity = ""; // Identity can be set based on the context if needed
        let ORKESTER_CREATE_COMPONENT_HANDLE = "orkester/CreateComponent";
        let ORKESTER_FORMAT_JSON = "std/json";
        let json_payload = serde_json::json!({"kind": parser_kind});
        let enveloppe = Envelope::new(0, identity, ORKESTER_CREATE_COMPONENT_HANDLE, ORKESTER_FORMAT_JSON, json_payload);
        self.host.handle(enveloppe)
    }

}
