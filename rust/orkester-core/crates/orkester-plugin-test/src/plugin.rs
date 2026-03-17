use orkester_plugin::sdk::{create_component_box, OwnedMessage, Plugin, Result, Host, Message};

use crate::{
    components,
    protocol::{
        ComponentDescriptor,
        CreateComponentRequest,
        PluginMetadata,
        MSG_CREATE_COMPONENT,
        MSG_GET_PLUGIN_METADATA,
        MSG_LIST_COMPONENTS,
    },
    util,
};

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
            MSG_GET_PLUGIN_METADATA => {
                let metadata = PluginMetadata {
                    name: "orkester-plugin-test".to_string(),
                    version: "0.1.0".to_string(),
                    description: "Example plugin for Orkester".to_string(),
                    authors: vec!["OpenAI".to_string()],
                    tags: vec!["example".to_string(), "test".to_string()],
                };

                util::json_response(request.id(), &metadata)
            }

            MSG_LIST_COMPONENTS => {
                let components = vec![
                    ComponentDescriptor {
                        id: "echo".to_string(),
                        description: "Returns the received payload unchanged".to_string(),
                    },
                    ComponentDescriptor {
                        id: "upper".to_string(),
                        description: "Returns the received UTF-8 payload uppercased".to_string(),
                    },
                    ComponentDescriptor {
                        id: "counter".to_string(),
                        description: "Stateful counter component".to_string(),
                    },
                ];

                util::json_response(request.id(), &components)
            }

            MSG_CREATE_COMPONENT => {
                let component = self.create_component(request)?;
                Ok(create_component_box(component))
            }

            _ => Ok(util::utf8_response(request.id(), "unsupported root request")),
        }
    }

    fn create_component(
        &mut self,
        request: Message<'_>,
    ) -> Result<Box<dyn orkester_plugin::sdk::Component>> {
        if request.type_id() != MSG_CREATE_COMPONENT {
            return Err(orkester_plugin::sdk::Error::Custom("invalid create request type"));
        }

        let create: CreateComponentRequest = serde_json::from_slice(request.payload())
            .map_err(|_| orkester_plugin::sdk::Error::Custom("invalid create request payload"))?;

        components::create_component(&create.component_id, &create.config)
            .ok_or(orkester_plugin::sdk::Error::Custom("unknown component"))
    }
}