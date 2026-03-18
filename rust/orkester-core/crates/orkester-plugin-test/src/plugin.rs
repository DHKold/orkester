use orkester_plugin::sdk::{
    create_component_box, Component, Host, Message, OwnedMessage, Plugin, Result,
};
use orkester_plugin::sdk::protocol::{
    CreateComponentRequest, PluginMetadata,
    MSG_CREATE_COMPONENT, MSG_GET_METADATA, MSG_LIST_COMPONENTS,
};

use crate::components;

pub struct TestPlugin {
    #[allow(dead_code)]
    host: Host,
}

impl Plugin for TestPlugin {
    fn new(host: Host) -> Result<Self> {
        Ok(Self { host })
    }

    fn handle(&mut self, request: Message<'_>) -> Result<OwnedMessage> {
        match request.type_id() {
            MSG_GET_METADATA => {
                let metadata = PluginMetadata {
                    name: "orkester-plugin-test".to_string(),
                    version: "0.1.0".to_string(),
                    description: Some("Example plugin for Orkester".to_string()),
                    authors: vec!["OpenAI".to_string()],
                    tags: vec!["example".to_string(), "test".to_string()],
                    extra: serde_json::Map::new(),
                };

                json_response(request.id(), MSG_GET_METADATA, &metadata)
            }

            MSG_LIST_COMPONENTS => {
                let components = vec![
                    components::counter::CounterComponent::metadata(),
                    components::upper::UpperComponent::metadata(),
                    components::echo::EchoComponent::metadata(),
                ];

                json_response(request.id(), MSG_LIST_COMPONENTS, &components)
            }

            MSG_CREATE_COMPONENT => {
                let component = self.create_component(request)?;
                Ok(create_component_box(component))
            }

            _ => json_error(request.id(), "unsupported_request", "unsupported root request"),
        }
    }

    fn create_component(&mut self, request: Message<'_>) -> Result<Box<dyn Component>> {
        let create: CreateComponentRequest = decode_json(request)?;

        components::create_component(&create.component_id, &create.config)
            .ok_or(orkester_plugin::sdk::Error::Custom("unknown component"))
    }
}