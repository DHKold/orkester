use orkester_plugin::prelude::*;
use crate::{
    calculator::CalculatorComponent,
    counter::{self, CounterComponent},
    echo::{self, EchoComponent},
    logger::{self, LoggerComponent},
};

/// Root component — the plugin entry point.
///
/// Exposes factories for every component kind in this plugin.
#[derive(Default)]
pub struct RootComponent;

#[component(
    kind        = "sample/Root:1.0",
    name        = "SampleRoot",
    description = "Root component for the Orkester sample plugin."
)]
impl RootComponent {
    #[factory("sample/Logger:1.0")]
    fn make_logger(&mut self, config: logger::LoggerConfig) -> Result<LoggerComponent> {
        LoggerComponent::new(config)
    }

    #[factory("sample/Calculator:1.0")]
    fn make_calculator(&mut self, _config: ()) -> Result<CalculatorComponent> {
        Ok(CalculatorComponent::default())
    }

    #[factory("sample/Counter:1.0")]
    fn make_counter(&mut self, config: counter::CounterConfig) -> Result<CounterComponent> {
        Ok(CounterComponent::new(config))
    }

    #[factory("sample/Echo:1.0")]
    fn make_echo(&mut self, config: echo::EchoConfig) -> Result<EchoComponent> {
        Ok(EchoComponent::new(config))
    }
}
