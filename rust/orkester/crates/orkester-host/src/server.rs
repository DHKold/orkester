use orkester_plugin::abi::AbiHost;
use orkester_plugin::prelude::*;

pub struct HostServer{
    catalog: Catalog,
    registry: Arc<Mutex<Vec<ComponentEntry>>>,
}

#[component(
    kind = "orkester/HostServer:1.0",
    name = "Orkester Host Server",
    description = "The main server component for Orkester Host, responsible for managing plugins and communication."
)]
impl HostServer {
    /// Create a new HostServer instance.
    fn new(catalog: Catalog, registry: Arc<Mutex<Vec<ComponentEntry>>>) -> Self {
        Self { catalog, registry }
    }

    #[handle("orkester/CreateComponent:1.0")]
    fn create_component(&mut self, request: CreateComponentRequest) -> Result<CreateComponentResponse, HostError> {
        catalog::instantiate_component(&mut self.catalog, &request.config)
            .map_err(|e| HostError::ComponentCreationFailed(request.config.kind.clone(), e.to_string()))
            .and_then(|component| {
                registry::register_component(&self.registry, component.clone());
                Ok(CreateComponentResponse { component: component.ptr() })
            })
    }
}