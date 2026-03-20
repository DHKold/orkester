//! In-memory counter with increment, decrement, reset, and get operations.

use orkester_plugin::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Deserialize)]
pub struct CounterConfig {
    /// Initial value for the counter. Defaults to 0.
    #[serde(default)]
    pub initial: i64,
    /// Optional lower bound (inclusive). Returns error if decrement would go below.
    pub min: Option<i64>,
    /// Optional upper bound (inclusive). Returns error if increment would go above.
    pub max: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct CounterState {
    pub value: i64,
}

#[derive(Debug, Deserialize)]
pub struct StepRequest {
    /// Number of units to add (positive) or subtract (negative).
    #[serde(default = "one")]
    pub step: i64,
}

fn one() -> i64 { 1 }

pub struct CounterComponent {
    value: i64,
    min: Option<i64>,
    max: Option<i64>,
}

impl CounterComponent {
    pub fn new(config: CounterConfig) -> Self {
        Self { value: config.initial, min: config.min, max: config.max }
    }
}

#[component(
    kind        = "sample/Counter:1.0",
    name        = "Counter",
    description = "An in-memory integer counter with optional min/max bounds."
)]
impl CounterComponent {
    /// Get the current value.
    #[handle("sample/Counter/Get")]
    fn get(&mut self, _: ()) -> Result<CounterState> {
        Ok(CounterState { value: self.value })
    }

    /// Increment by `step` (default 1). Returns the new value.
    #[handle("sample/Counter/Increment")]
    fn increment(&mut self, req: StepRequest) -> Result<CounterState> {
        let next = self.value + req.step;
        if let Some(max) = self.max {
            if next > max {
                return Err(format!("counter would exceed maximum ({max})").into());
            }
        }
        self.value = next;
        Ok(CounterState { value: self.value })
    }

    /// Decrement by `step` (default 1). Returns the new value.
    #[handle("sample/Counter/Decrement")]
    fn decrement(&mut self, req: StepRequest) -> Result<CounterState> {
        let next = self.value - req.step;
        if let Some(min) = self.min {
            if next < min {
                return Err(format!("counter would go below minimum ({min})").into());
            }
        }
        self.value = next;
        Ok(CounterState { value: self.value })
    }

    /// Reset the counter to its initial value (0 unless configured otherwise).
    #[handle("sample/Counter/Reset")]
    fn reset(&mut self, _: ()) -> Result<CounterState> {
        self.value = 0;
        Ok(CounterState { value: self.value })
    }
}
