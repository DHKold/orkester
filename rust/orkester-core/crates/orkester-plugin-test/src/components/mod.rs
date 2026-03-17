pub mod counter;
pub mod echo;
pub mod upper;

use orkester_plugin::sdk::Component;
use serde_json::Value;

use self::{
    counter::CounterComponent,
    echo::EchoComponent,
    upper::UpperComponent,
};

pub fn create_component(
    component_id: &str,
    config: &Value,
) -> Option<Box<dyn Component>> {
    match component_id {
        "echo" => Some(Box::new(EchoComponent::new())),
        "upper" => Some(Box::new(UpperComponent::new())),
        "counter" => {
            let initial = config
                .get("initial")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0);

            Some(Box::new(CounterComponent::new(initial)))
        }
        _ => None,
    }
}