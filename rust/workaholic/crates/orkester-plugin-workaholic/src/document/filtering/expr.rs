//! Query expression tree for document filtering.

use serde::{Deserialize, Serialize};
use serde_json::Value;

// ─── Query ────────────────────────────────────────────────────────────────────

/// A composable filter expression.
///
/// JSON shape (snake_case, externally-tagged):
/// ```json
/// { "all": [ { "when": { "eq": { "lhs": { "field": "kind" }, "rhs": { "value": "Task" } } } } ] }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Query {
    /// All sub-queries must match (AND).
    All(Vec<Query>),
    /// At least one sub-query must match (OR).
    Any(Vec<Query>),
    /// Exactly one sub-query must match (XOR).
    OneOf(Vec<Query>),
    /// Sub-query must not match (NOT).
    Not(Box<Query>),
    /// Leaf: a single criterion evaluated against the document.
    When(Criterion),
}

// ─── Criterion ────────────────────────────────────────────────────────────────

/// A single comparison. Both sides are [`Operand`]s, allowing field-vs-field
/// comparisons and computed values in addition to the common field-vs-literal case.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Criterion {
    /// `lhs == rhs` — true when both are absent (both `None`).
    Eq  { lhs: Operand, rhs: Operand },
    /// `lhs != rhs` — false when `lhs` is absent.
    Neq { lhs: Operand, rhs: Operand },
    /// `lhs < rhs`  (numeric f64 or lexicographic string).
    Lt  { lhs: Operand, rhs: Operand },
    /// `lhs <= rhs`
    Lte { lhs: Operand, rhs: Operand },
    /// `lhs > rhs`
    Gt  { lhs: Operand, rhs: Operand },
    /// `lhs >= rhs`
    Gte { lhs: Operand, rhs: Operand },
    /// `lhs` (string) matches the `pattern` (string) as a regex.
    Regex   { lhs: Operand, pattern: Operand },
    /// `lhs` (array) contains `element`, or `lhs` (string) contains `element` as a substring.
    Contains { lhs: Operand, element: Operand },
    /// Operand resolves to a value (including `null`).
    Exists(Operand),
    /// Operand does not resolve (path absent from document).
    Missing(Operand),
}

// ─── Operand ──────────────────────────────────────────────────────────────────

/// A value source for a criterion operand.
///
/// | Variant | JSON | Resolves to |
/// |---------|------|-------------|
/// | `Field`  | `{ "field": "metadata.namespace" }` | value at dot-path, or absent |
/// | `Value`  | `{ "value": "testing" }` | the literal; always present |
/// | `Concat` | `{ "concat": [op, …] }` | string concatenation of parts |
/// | `Sum`    | `{ "sum": [op, …] }` | numeric sum of parts |
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Operand {
    /// Dot-separated path into the document, e.g. `"metadata.namespace"`.
    Field(String),
    /// A literal JSON value.
    Value(Value),
    /// Concatenate operands as strings. Any non-string part causes the operand to be absent.
    Concat(Vec<Operand>),
    /// Sum operands as `f64`. Any non-numeric part causes the operand to be absent.
    Sum(Vec<Operand>),
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Full round-trip: JSON → Query → JSON. Locks the REST API contract.
    /// Any accidental rename_all change will break this test.
    #[test]
    fn json_roundtrip_compound_query() {
        let json_in = json!({
            "all": [
                { "when": { "eq": { "lhs": { "field": "kind" }, "rhs": { "value": "workaholic/Task:1.0" } } } },
                { "any": [
                    { "when": { "eq":       { "lhs": { "field": "metadata.namespace" }, "rhs": { "value": "testing" } } } },
                    { "when": { "contains": { "lhs": { "field": "metadata.tags" }, "element": { "value": "ci" } } } }
                ]},
                { "not": { "when": { "exists": { "field": "spec.deprecated" } } } }
            ]
        });
        let query: Query   = serde_json::from_value(json_in.clone()).expect("deserialization must succeed");
        let json_out: Value = serde_json::to_value(&query).expect("serialization must succeed");
        assert_eq!(json_in, json_out, "round-trip must be lossless and key-stable");
    }

    #[test]
    fn operand_variants_serialize_correctly() {
        assert_eq!(serde_json::to_value(Operand::Field("a.b".into())).unwrap(), json!({ "field": "a.b" }));
        assert_eq!(serde_json::to_value(Operand::Value(json!(42))).unwrap(),    json!({ "value": 42 }));
        assert_eq!(
            serde_json::to_value(Operand::Concat(vec![Operand::Field("x".into()), Operand::Value(json!("-"))])).unwrap(),
            json!({ "concat": [{ "field": "x" }, { "value": "-" }] })
        );
    }

    #[test]
    fn criterion_struct_variants_serialize_as_single_key_objects() {
        let c = Criterion::Eq { lhs: Operand::Field("name".into()), rhs: Operand::Value(json!("x")) };
        assert_eq!(serde_json::to_value(&c).unwrap(), json!({ "eq": { "lhs": { "field": "name" }, "rhs": { "value": "x" } } }));
    }

    #[test]
    fn criterion_unit_like_variants_serialize_correctly() {
        let exists  = Criterion::Exists(Operand::Field("spec".into()));
        let missing = Criterion::Missing(Operand::Field("spec".into()));
        assert_eq!(serde_json::to_value(&exists).unwrap(),  json!({ "exists":  { "field": "spec" } }));
        assert_eq!(serde_json::to_value(&missing).unwrap(), json!({ "missing": { "field": "spec" } }));
    }

    #[test]
    fn cross_field_criterion_roundtrips() {
        let json_in = json!({ "when": { "eq": { "lhs": { "field": "spec.owner" }, "rhs": { "field": "metadata.owner" } } } });
        let q: Query = serde_json::from_value(json_in.clone()).unwrap();
        assert_eq!(serde_json::to_value(&q).unwrap(), json_in);
    }
}
