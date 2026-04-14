//! Generic document filtering — composable query expressions over JSON documents.
//!
//! | Module     | Responsibility                                            |
//! |------------|-----------------------------------------------------------|
//! | `expr`     | `Query`, `Criterion`, `Operand` — the expression tree     |
//! | `field`    | Dot-path resolution into `serde_json::Value`              |
//! | `operand`  | `Operand` resolution: field, literal, concat, sum         |
//! | `compare`  | `Criterion` evaluation against a document                 |
//! | `eval`     | Recursive `apply` + `filter` convenience functions        |

mod compare;
mod eval;
mod expr;
mod field;
mod operand;

pub use eval::{apply, filter};
pub use expr::{Criterion, Operand, Query};

