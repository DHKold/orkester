// Actions
pub const ACTION_LOAD_DOCUMENTS:        &str = "workaholic/DocumentLoader/SearchDocuments";
pub const ACTION_LOADER_START:          &str = "workaholic/DocumentLoader/Start";
pub const ACTION_LOADER_CREATE_ENTRY:   &str = "workaholic/DocumentLoader/CreateEntry";
pub const ACTION_LOADER_RETRIEVE_ENTRY: &str = "workaholic/DocumentLoader/RetrieveEntry";
pub const ACTION_LOADER_UPDATE_ENTRY:   &str = "workaholic/DocumentLoader/UpdateEntry";
pub const ACTION_LOADER_DELETE_ENTRY:   &str = "workaholic/DocumentLoader/DeleteEntry";
pub const ACTION_LOADER_SEARCH_ENTRIES: &str = "workaholic/DocumentLoader/SearchEntries";

// Events
pub const EVENT_LOADER_DOCUMENT_ADDED:    &str = "workaholic/DocumentLoader/Event/DocumentAdded";
pub const EVENT_LOADER_DOCUMENT_REMOVED:  &str = "workaholic/DocumentLoader/Event/DocumentRemoved";
pub const EVENT_LOADER_DOCUMENT_MODIFIED: &str = "workaholic/DocumentLoader/Event/DocumentModified";