use orkester::prelude::*;
use workaholic::document::Document;

pub const ACTION_CATALOG_CREATE_RESOURCE: &str = "workaholic/CatalogServer/CreateResource";
pub const ACTION_CATALOG_RETRIEVE_RESOURCE: &str = "workaholic/CatalogServer/RetrieveResource";
pub const ACTION_CATALOG_UPDATE_RESOURCE: &str = "workaholic/CatalogServer/UpdateResource";
pub const ACTION_CATALOG_DELETE_RESOURCE: &str = "workaholic/CatalogServer/DeleteResource";
pub const ACTION_CATALOG_SEARCH_RESOURCES: &str = "workaholic/CatalogServer/SearchResources";

/// A simple in-memory catalog server that manages generic resources represented as Documents.
/// Will be extended in the future to support more advanced features like persistence, indexing, access control, etc.
pub struct CatalogServer {
    storage: HashMap<String, Box<dyn Document>>,
}

#[component(
    kind = "workaholic/CatalogServer:1.0",
    name = "Workaholic Resources Catalog Server",
    description = "A server that provides a catalog of generic resources for the Workaholic plugin.",
)]
pub impl CatalogServer {
    /// Initializes the catalog server with an empty storage.
    fn new() -> Self {
        Self {
            storage: HashMap::new(),
        }
    }

    /// Creates a new resource in the catalog.
    #[handle(ACTION_CATALOG_CREATE_RESOURCE)]
    fn create_resource(&self, resource: Box<dyn Document>) -> Box<dyn Document> {
        let id = format!("{}:{}", resource.kind, resource.name);
        self.storage.insert(id, resource.clone());
        resource
    }

    /// Retrieves a resource from the catalog by its ID.
    #[handle(ACTION_CATALOG_RETRIEVE_RESOURCE)]
    fn get_resource(&self, id: String) -> Option<Box<dyn Document>> {
        self.storage.get(&id).cloned()
    }

    /// Updates an existing resource in the catalog.
    #[handle(ACTION_CATALOG_UPDATE_RESOURCE)]
    fn update_resource(&self, id: String, resource: Box<dyn Document>) -> Option<Box<dyn Document>> {
        if self.storage.contains_key(&id) {
            self.storage.insert(id.clone(), resource.clone());
            Some(resource)
        } else {
            None
        }
    }

    /// Deletes a resource from the catalog by its ID.
    #[handle(ACTION_CATALOG_DELETE_RESOURCE)]
    fn delete_resource(&self, id: String) -> bool {
        self.storage.remove(&id).is_some()
    }

    /// Searches for resources in the catalog based on a query string.
    #[handle(ACTION_CATALOG_SEARCH_RESOURCES)]
    fn search_resources(&self, query: String) -> Vec<Box<dyn Document>> {
        // For now we just allow searching by kind, name or metadata.namespace. Query format is `<field>=<value>`, e.g. `kind=orkester/task:1.0` or `name=my-task`.
        let mut results = Vec::new();
        for resource in self.storage.values() {
            if query.starts_with("kind=") && resource.kind == query[5..] {
                results.push(resource.clone());
            } else if query.starts_with("name=") && resource.name == query[5..] {
                results.push(resource.clone());
            } else if query.starts_with("namespace=") && resource.metadata.namespace.as_deref() == Some(&query[10..]) {
                results.push(resource.clone());
            }
        }
        results
    }

    /// Request to load documents.
    #[handle(ACTION_CATALOG_LOAD_DOCUMENTS)]
    fn load_documents(&self, request: CatalogLoadDocumentsRequest) -> Result<(), LoadDocumentsError> {
        let loader: ComponentsLoader = get_component<ComponentsLoader>(&request.loaderRef)
            .ok_or_else(|| LoadDocumentsError::LoaderNotFound(request.loaderRef.clone()))?;
        let documents = loader.load_documents().map_err(|e| LoadDocumentsError::LoaderError(request.loaderRef.clone(), e.to_string()))?;
        for doc in documents {
            self.create_resource(Box::new(doc));
        }
        Ok(())
    }
}