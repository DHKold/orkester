//! Four-operation calculator component.

use orkester_plugin::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct CalcRequest {
    pub op: Op,
    pub a: f64,
    pub b: f64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Op {
    Add,
    Sub,
    Mul,
    Div,
}

#[derive(Debug, Serialize)]
pub struct CalcResponse {
    pub result: f64,
}

#[derive(Default)]
pub struct CalculatorComponent;

#[component(
    kind        = "sample/Calculator:1.0",
    name        = "Calculator",
    description = "Performs basic arithmetic operations on two f64 operands."
)]
impl CalculatorComponent {
    #[handle("sample/Calculate")]
    fn calculate(&mut self, req: CalcRequest) -> Result<CalcResponse> {
        let result = match req.op {
            Op::Add => req.a + req.b,
            Op::Sub => req.a - req.b,
            Op::Mul => req.a * req.b,
            Op::Div => {
                if req.b == 0.0 {
                    return Err("division by zero".into());
                }
                req.a / req.b
            }
        };
        Ok(CalcResponse { result })
    }
}
