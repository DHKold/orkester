//==== EXAMPLE USAGE OF THE SDK IN A PLUGIN (echo_plugin.so) ====
mod echo_plugin {
    use orkester_plugin::{abi, sdk};
    // Other uses...

    struct EchoComponent {
        host: sdk::Host,
    }

    #[derive(Deserialize)]
    struct EchoRequest {
        message: String,
    }

    #[derive(Serialize)]
    struct EchoResponse {
        message: String,
    }

    pub impl EchoComponent {
        pub fn echo(&mut self, request: EchoRequest) -> Result<EchoResponse> {
            Ok(EchoResponse { message: request.message })
        }

        // Example of a method that creates a subcomponent. The sdk will need to package the `TotoComponent` into an `AbiComponent` and return it to the host, which can then call methods on it.
        pub fn make_toto(&mut self, config: TotoComponentConfig) -> Result<TotoComponent> {
            // Example of sending a message to the host from within a component method, which can be used for logging, telemetry, or other interactions with the host.
            self.host.handle(Message { kind: "orkester/Log", payload: "Creating TotoComponent".into() })?;
            Ok(TotoComponent::new(config))
        }

        // Example of a custom serializer: the sdk will use this to serialize `SomeType` values when returned from component methods or sent to the host.
        pub fn serialize_some_type(&self, value: SomeType) -> Result<Vec<u8>> {
            Ok(value.serialize_to_bytes())
        }

        // Example of a custom deserializer: the sdk will use this to deserialize a payload with specific format (id) into a `CustomType`
        pub fn deserialize_custom_format(&self, message: abi::AbiRequest) -> Result<CustomType> {
            CustomType::deserialize_from_bytes(message.payload, message.len, message.flags)?
        }
    }

    impl sdk::PluginComponent for EchoComponent {
        pub fn get_metadata() -> sdk::ComponentMetadata {
            sdk::ComponentMetadata {
                kind: "example/EchoComponent:1.0".to_string(),
                name: "EchoComponent".to_string(),
                description: "A simple component that echoes messages back to the host.".to_string(),
            }
        }

        pub fn to_abi(self) -> abi::AbiComponent {
            sdk::AbiComponentBuilder::new(self.host)
                 .with_handler("example/echo", Self::echo) // Register the `echo` method as a handler for requests with the "example/echo" action.
                 .with_handler("orkester/GetMetadata", Self::get_metadata) // Register the `get_metadata` method as a handler for the standard "GetMetadata" action, which allows the host to query the component's metadata.
                 .with_factory("example/TotoComponent:1.2", Self::make_toto, TotoComponent::get_metadata) // Register the `make_toto` method as a factory for creating `TotoComponent` subcomponents.
                 .with_serializer::<SomeType>("example/SomeType", Self::serialize_some_type) // Register the `serialize_some_type` method as a custom serializer for `SomeType` values.
                 .with_deserializer::<CustomType>("example/CustomType", Self::deserialize_custom_format) // Register the `deserialize_custom_format` method as a custom deserializer for payloads with the "example/CustomType" format.
                 .build(self) // Build the ABI component, which will generate the appropriate handle and free functions that dispatch to the registered handlers.
        }
    }

    unsafe extern "C" fn orkester_plugin_entry(host: *mut abi::AbiHost) -> *mut abi::AbiComponent {
        let root = EchoComponent {
            host: sdk::Host::from_abi(host),
        };
        Box::into_raw(Box::new(root.to_abi()))
    }
}

//==== TARGET USAGE: HOST AUTHORS ====

mod host_usage_example {
    use orkester_plugin::{abi, sdk};

    // Loading a plugin
    let mut host = sdk::Host::new();
    let plugin = host.load_plugin("path/to/plugin.so")?;

    // Using generic host calls
    let raw_root_component: abi::AbiComponent = plugin.get_root_component();
    let raw_response: abi::AbiResponse = raw_root_component.handle(&raw_root_component, sdk::message::Serializer::json("Hello, world!"));
    let response: Value = sdk::message::Deserializer::value(raw_response);
    let raw_subcomponent: abi::AbiComponent = raw_root_component.handle(
        &raw_root_component,
        sdk::message::Serializer::json(sdk::message::CreateComponentRequest::new("example/TotoComponent:1.2").with_config(some_config)),
    ) |> sdk::message::Deserializer::component(raw_response);
    let subcomponent_response: String = raw_subcomponent.handle(
        &raw_subcomponent, 
        sdk::message::Serializer::json(sdk::message::Request::new("example/MyTotoRequest", some_params))
    ) |> sdk::message::Deserializer::string(raw_response);
}
