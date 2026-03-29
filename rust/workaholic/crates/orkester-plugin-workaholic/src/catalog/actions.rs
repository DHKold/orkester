pub const ACTION_CATALOG_CREATE_RESOURCE:   &str = "workaholic/CatalogServer/CreateResource";
pub const ACTION_CATALOG_RETRIEVE_RESOURCE: &str = "workaholic/CatalogServer/RetrieveResource";
pub const ACTION_CATALOG_UPDATE_RESOURCE:   &str = "workaholic/CatalogServer/UpdateResource";
pub const ACTION_CATALOG_DELETE_RESOURCE:   &str = "workaholic/CatalogServer/DeleteResource";
pub const ACTION_CATALOG_SEARCH_RESOURCES:  &str = "workaholic/CatalogServer/SearchResources";
pub const ACTION_CATALOG_LOAD_DOCUMENTS:    &str = "workaholic/CatalogServer/LoadDocuments";

/// List all Namespace documents in the catalog.
pub const ACTION_CATALOG_LIST_NAMESPACES: &str = "workaholic/CatalogServer/ListNamespaces";
/// List all Work documents in a given namespace.
pub const ACTION_CATALOG_LIST_WORKS:      &str = "workaholic/CatalogServer/ListWorks";
/// List all Task documents in a given namespace.
pub const ACTION_CATALOG_LIST_TASKS:      &str = "workaholic/CatalogServer/ListTasks";