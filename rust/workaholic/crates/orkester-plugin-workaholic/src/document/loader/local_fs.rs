pub struct LocalFsLoader {
    // Loading parameters
    paths: Vec<String>,
    recursive: bool,
    extensions: HashMap<String, DocumentParser>,
}

pub impl LocalFsLoader {
    /// Create a new LocalFsLoader with the given file paths.
    pub fn new(paths: Vec<String>, recursive: bool, extensions: HashMap<String, DocumentParser>) -> Self {
        Self { paths, recursive, extensions }
    }

    /// Load documents from a single path, which can be a file or a directory.
    fn load_from_path(&self, path: &str) -> Result<Vec<Document>> {
        // Check if the path exists and determine if it's a file or directory
        let metadata = std::fs::metadata(path).map_err(|e| LoaderError::WrongPath(path.to_string(), e.to_string()))?;
        if metadata.is_file() {
            self.load_from_file(path)
        } else if metadata.is_dir() {
            self.load_from_directory(path)
        } else {
            Err(LoaderError::WrongPath(path.to_string(), "Not a file or directory".to_string()).into())
        }
    }

    /// Load a document from a single file, checking the extension and parsing the content.
    fn load_from_file(&self, file_path: &str) -> Result<Vec<Document>> {
        // Check file extension
        let supported_extensions = self.extensions.keys().cloned().collect::<Vec<_>>().join(", ");
        let ext = std::path::Path::new(file_path)
            .extension()
            .and_then(|s| s.to_str())
            .ok_or_else(|| LoaderError::WrongFileExtension(file_path.to_string(), supported_extensions.clone()))?;
        if !self.extensions.contains_key(ext) {
            return Err(LoaderError::WrongFileExtension(file_path.to_string(), supported_extensions.clone()).into());
        }

        // Load file content
        let content = std::fs::read_to_string(file_path).map_err(|e| LoaderError::ReadError(file_path.to_string(), e.to_string()))?;
        let documents: Vec<Document> = self.parse_content(&content, ext)?;
        Ok(documents)
    }

    /// Load documents from a directory, optionally scanning recursively, and filtering by file extensions.
    fn load_from_directory(&self, dir_path: &str) -> Result<Vec<Document>> {
        let mut documents = Vec::new();
        let entries = std::fs::read_dir(dir_path).map_err(|e| LoaderError::WrongPath(dir_path.to_string(), e.to_string()))?;
        for entry in entries {
            let entry = entry.map_err(|e| LoaderError::WrongPath(dir_path.to_string(), e.to_string()))?;
            let path = entry.path().to_str().ok_or_else(|| LoaderError::WrongPath(dir_path.to_string(), "Invalid path".to_string()))?;
            documents.extend(self.load_from_path(path)?);
        }
        Ok(documents)
    }

    /// Parse the content of a file into a Document struct using the appropriate parser based on the file extension.
    fn parse_content(&self, content: &str, ext: &str) -> Result<Vec<Document>> {
        let parser = self.extensions.get(ext).ok_or_else(|| LoaderError::WrongFileExtension(ext.to_string(), "Unsupported extension".to_string()))?;
        parser.parse(content)
    }
}

pub impl DocumentLoader for LocalFsLoader {
    /// Load documents from the local filesystem based on the configured paths and parameters.
    /// Each path can be a file or a directory. If it's a directory, it will be scanned for files with the specified extensions.
    /// The `recursive` flag determines whether to scan directories recursively.
    /// Returns a vector of loaded documents or an error if any issues occur during loading.
    fn load(&self) -> Result<Vec<Document>> {
        let mut documents = Vec::new();
        for path in &self.paths {
            documents.extend(self.load_from_path(path)?);
        }
        Ok(documents)
    }
}

// === Export the component for use in Orkester ===
pub struct LocalFsLoaderComponent {
    loader: LocalFsLoader,
}

pub struct LocalFsLoaderConfig {
    // List of file or directory paths to load documents from.
    paths: Vec<String>,
    // Whether to scan directories recursively when loading from a directory path.
    recursive: bool,
    // Map of file extension to parser type (e.g. "yaml" -> "workaholic/YamlDocumentParser:1.0").
    extensions: HashMap<String, String>,
}

#[component(
    kind = "workaholic/LocalFsLoader:1.0",
    name = "Local Filesystem Loader",
    description = "Loader that reads documents from the local filesystem based on specified paths and parameters.",
)]
pub impl LocalFsLoaderComponent {
    /// Create a new LocalFsLoaderComponent with the given parameters.
    pub fn new(host: Host, config: LocalFsLoaderConfig) -> Self {
        let mut parsers = HashMap::new();
        for (ext, parser_kind) in &config.extensions {
            let parser = host.get_component(parser_kind).unwrap_or_else(|| panic!("Failed to get parser component for kind: {}", parser_kind));
            parsers.insert(ext.clone(), parser);
        }
        let loader = LocalFsLoader::new(config.paths, config.recursive, parsers);
        Self { loader }
    }

    /// Load documents using the underlying LocalFsLoader.
    #[handle(ACTION_LOAD_DOCUMENTS)]
    pub fn handle_load(&self) -> Result<Vec<Document>> {
        self.loader.load()
    }
}