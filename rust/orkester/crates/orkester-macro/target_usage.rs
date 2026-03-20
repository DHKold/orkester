use orkester_plugin::prelude::*;

pub struct EchoComponent{
    prefix: String,
}

pub struct EchoComponentConfig {
    pub prefix: String,
}

#[component(kind = "example/EchoComponent:1.0", name = "ExampleComponent1", description = "An example component that demonstrates the plugin SDK.")]
pub impl EchoComponent {
    fn new(config: EchoComponentConfig) -> Self {
        Self { prefix: config.prefix }
    }

    #[handle("example/EchoRequest")]
    fn echo(&mut self, request: EchoRequest) -> Result<EchoResponse> {
        Ok(EchoResponse { message: format!("{}{}", self.prefix, request.message) })
    }

    #[serializer(SomeType)]
    fn serialize_some_type(&self, value: SomeType) -> Result<*const u8> {
        Ok(value.serialize_to_bytes())
    }

    #[deserializer("custom/format:1.2")]
    fn deserialize_custom_format(&self, message: AbiRequest) -> Result<CustomType> {
        CustomType::deserialize_from_bytes(message.payload, message.len, message.flags)?
    }
}

#[component(kind = "example/RootComponent:1.0", name = "ExampleRootComponent", description = "The root component that serves as the plugin entry point.")]
pub impl RootComponent {
    #[factory("example/EchoComponent:1.0")]
    fn make_echo(&mut self, config: EchoComponentConfig) -> Result<EchoComponent> {
        Ok(EchoComponent::new(config))
    }
}

orkester_plugin::export_plugin_root!(components::RootComponent);