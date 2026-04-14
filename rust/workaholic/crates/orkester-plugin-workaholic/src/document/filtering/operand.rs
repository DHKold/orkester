//! Operand resolution — turns an [`Operand`] into an optional JSON value.

use std::borrow::Cow;

use serde_json::{Number, Value};

use super::expr::Operand;
use super::field::resolve;

/// Resolve an [`Operand`] against `doc`.
///
/// - `Field`  — borrows from the document; absent path → `None`.
/// - `Value`  — borrows the literal; always `Some`.
/// - `Concat` — allocates a new string; `None` if any part is not a string.
/// - `Sum`    — allocates a new number; `None` if any part is non-numeric or NaN/inf.
pub fn resolve_operand<'a>(doc: &'a Value, operand: &'a Operand) -> Option<Cow<'a, Value>> {
    match operand {
        Operand::Field(path)   => resolve(doc, path).map(Cow::Borrowed),
        Operand::Value(v)      => Some(Cow::Borrowed(v)),
        Operand::Concat(parts) => resolve_concat(doc, parts).map(Cow::Owned),
        Operand::Sum(parts)    => resolve_sum(doc, parts).map(Cow::Owned),
    }
}

fn resolve_concat(doc: &Value, parts: &[Operand]) -> Option<Value> {
    let s = parts
        .iter()
        .map(|op| resolve_operand(doc, op)?.as_ref().as_str().map(str::to_owned))
        .collect::<Option<Vec<_>>>()?
        .join("");
    Some(Value::String(s))
}

fn resolve_sum(doc: &Value, parts: &[Operand]) -> Option<Value> {
    let mut total = 0.0f64;
    for op in parts {
        total += resolve_operand(doc, op)?.as_ref().as_f64()?;
    }
    Number::from_f64(total).map(Value::Number)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn field_borrows_from_doc() {
        let doc = json!({ "metadata": { "namespace": "testing" } });
        let op  = Operand::Field("metadata.namespace".into());
        assert_eq!(resolve_operand(&doc, &op).as_deref(), Some(&json!("testing")));
    }

    #[test]
    fn field_absent_returns_none() {
        let doc = json!({});
        let op  = Operand::Field("missing".into());
        assert_eq!(resolve_operand(&doc, &op), None);
    }

    #[test]
    fn value_always_resolves() {
        let doc = json!({});
        let op  = Operand::Value(json!(42));
        assert_eq!(resolve_operand(&doc, &op).as_deref(), Some(&json!(42)));
    }

    #[test]
    fn concat_joins_strings() {
        let doc = json!({ "first": "hello", "second": "world" });
        let op  = Operand::Concat(vec![
            Operand::Field("first".into()),
            Operand::Value(json!("-")),
            Operand::Field("second".into()),
        ]);
        assert_eq!(resolve_operand(&doc, &op).as_deref(), Some(&json!("hello-world")));
    }

    #[test]
    fn concat_non_string_part_returns_none() {
        let doc = json!({ "n": 42 });
        let op  = Operand::Concat(vec![Operand::Field("n".into()), Operand::Value(json!("x"))]);
        assert_eq!(resolve_operand(&doc, &op), None);
    }

    #[test]
    fn sum_adds_numbers() {
        let doc = json!({ "a": 3, "b": 4 });
        let op  = Operand::Sum(vec![Operand::Field("a".into()), Operand::Field("b".into())]);
        assert_eq!(resolve_operand(&doc, &op).as_deref(), Some(&json!(7.0)));
    }

    #[test]
    fn sum_empty_returns_zero() {
        let doc = json!({});
        let op  = Operand::Sum(vec![]);
        assert_eq!(resolve_operand(&doc, &op).as_deref(), Some(&json!(0.0)));
    }

    #[test]
    fn sum_non_numeric_part_returns_none() {
        let doc = json!({ "s": "text" });
        let op  = Operand::Sum(vec![Operand::Field("s".into()), Operand::Value(json!(1))]);
        assert_eq!(resolve_operand(&doc, &op), None);
    }
}
