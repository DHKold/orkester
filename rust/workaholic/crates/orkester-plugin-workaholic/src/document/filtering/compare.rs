//! Criterion evaluation against a document.

use std::borrow::Cow;
use std::cmp::Ordering;

use serde_json::Value;

use super::expr::{Criterion, Operand};
use super::operand::resolve_operand;

/// Evaluate `criterion` against `doc`. Returns `true` if the criterion is satisfied.
pub fn evaluate(doc: &Value, criterion: &Criterion) -> bool {
    match criterion {
        Criterion::Exists(op)              => res(doc, op).is_some(),
        Criterion::Missing(op)             => res(doc, op).is_none(),
        Criterion::Eq  { lhs, rhs }        => res(doc, lhs) == res(doc, rhs),
        Criterion::Neq { lhs, rhs }        => { let l = res(doc, lhs); l.is_some() && l != res(doc, rhs) }
        Criterion::Lt  { lhs, rhs }        => cmp(doc, lhs, rhs).is_some_and(Ordering::is_lt),
        Criterion::Lte { lhs, rhs }        => cmp(doc, lhs, rhs).is_some_and(Ordering::is_le),
        Criterion::Gt  { lhs, rhs }        => cmp(doc, lhs, rhs).is_some_and(Ordering::is_gt),
        Criterion::Gte { lhs, rhs }        => cmp(doc, lhs, rhs).is_some_and(Ordering::is_ge),
        Criterion::Regex   { lhs, pattern }  => apply_regex(doc, lhs, pattern),
        Criterion::Contains { lhs, element } => apply_contains(doc, lhs, element),
    }
}

fn res<'a>(doc: &'a Value, op: &'a Operand) -> Option<Cow<'a, Value>> {
    resolve_operand(doc, op)
}

/// Order two operands numerically (f64) or lexicographically (string).
/// Returns `None` for absent operands or incompatible types.
fn cmp<'a>(doc: &'a Value, lhs: &'a Operand, rhs: &'a Operand) -> Option<Ordering> {
    match (res(doc, lhs)?.as_ref(), res(doc, rhs)?.as_ref()) {
        (Value::Number(a), Value::Number(b)) => a.as_f64()?.partial_cmp(&b.as_f64()?),
        (Value::String(a), Value::String(b)) => Some(a.cmp(b)),
        _ => None,
    }
}

fn apply_regex(doc: &Value, lhs: &Operand, pattern: &Operand) -> bool {
    let (Some(lval), Some(pval)) = (res(doc, lhs), res(doc, pattern)) else { return false; };
    let (Some(s), Some(pat)) = (lval.as_ref().as_str(), pval.as_ref().as_str()) else { return false; };
    regex::Regex::new(pat).map(|re| re.is_match(s)).unwrap_or(false)
}

fn apply_contains(doc: &Value, lhs: &Operand, element: &Operand) -> bool {
    let (Some(lval), Some(eval)) = (res(doc, lhs), res(doc, element)) else { return false; };
    match lval.as_ref() {
        Value::Array(arr) => arr.contains(eval.as_ref()),
        Value::String(s)  => eval.as_ref().as_str().map(|sub| s.contains(sub)).unwrap_or(false),
        _                 => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn field(path: &str) -> Operand { Operand::Field(path.into()) }
    fn val(v: serde_json::Value) -> Operand { Operand::Value(v) }

    #[test]
    fn eq_field_vs_literal() {
        let doc = json!({ "name": "t1" });
        assert!( evaluate(&doc, &Criterion::Eq { lhs: field("name"), rhs: val(json!("t1")) }));
        assert!(!evaluate(&doc, &Criterion::Eq { lhs: field("name"), rhs: val(json!("t2")) }));
        assert!(!evaluate(&doc, &Criterion::Eq { lhs: field("missing"), rhs: val(json!("t1")) }));
    }

    #[test]
    fn eq_field_vs_field() {
        let doc = json!({ "a": 5, "b": 5, "c": 6 });
        assert!( evaluate(&doc, &Criterion::Eq { lhs: field("a"), rhs: field("b") }));
        assert!(!evaluate(&doc, &Criterion::Eq { lhs: field("a"), rhs: field("c") }));
    }

    #[test]
    fn neq_false_for_absent_lhs() {
        let doc = json!({ "x": "y" });
        assert!(!evaluate(&doc, &Criterion::Neq { lhs: field("missing"), rhs: val(json!("y")) }));
        assert!( evaluate(&doc, &Criterion::Neq { lhs: field("x"),       rhs: val(json!("z")) }));
    }

    #[test]
    fn numeric_ordering() {
        let doc = json!({ "n": 5 });
        assert!( evaluate(&doc, &Criterion::Gt  { lhs: field("n"), rhs: val(json!(4)) }));
        assert!(!evaluate(&doc, &Criterion::Gt  { lhs: field("n"), rhs: val(json!(5)) }));
        assert!( evaluate(&doc, &Criterion::Gte { lhs: field("n"), rhs: val(json!(5)) }));
        assert!( evaluate(&doc, &Criterion::Lt  { lhs: field("n"), rhs: val(json!(6)) }));
        assert!( evaluate(&doc, &Criterion::Lte { lhs: field("n"), rhs: val(json!(5)) }));
    }

    #[test]
    fn string_ordering() {
        let doc = json!({ "s": "b" });
        assert!( evaluate(&doc, &Criterion::Gt { lhs: field("s"), rhs: val(json!("a")) }));
        assert!( evaluate(&doc, &Criterion::Lt { lhs: field("s"), rhs: val(json!("c")) }));
        assert!(!evaluate(&doc, &Criterion::Gt { lhs: val(json!(1)), rhs: val(json!("a")) })); // type mismatch
    }

    #[test]
    fn exists_and_missing() {
        let doc = json!({ "x": "v" });
        assert!( evaluate(&doc, &Criterion::Exists(field("x"))));
        assert!(!evaluate(&doc, &Criterion::Exists(field("y"))));
        assert!( evaluate(&doc, &Criterion::Missing(field("y"))));
        assert!(!evaluate(&doc, &Criterion::Missing(field("x"))));
    }

    #[test]
    fn regex_on_string_field() {
        let doc = json!({ "name": "hello-world" });
        assert!( evaluate(&doc, &Criterion::Regex { lhs: field("name"), pattern: val(json!("hello.*")) }));
        assert!(!evaluate(&doc, &Criterion::Regex { lhs: field("name"), pattern: val(json!("bye.*"))   }));
        assert!(!evaluate(&doc, &Criterion::Regex { lhs: field("missing"), pattern: val(json!(".*"))   }));
    }

    #[test]
    fn contains_array_and_string() {
        let doc = json!({ "tags": ["a", "b"], "title": "hello world" });
        assert!( evaluate(&doc, &Criterion::Contains { lhs: field("tags"),  element: val(json!("b"))     }));
        assert!(!evaluate(&doc, &Criterion::Contains { lhs: field("tags"),  element: val(json!("c"))     }));
        assert!( evaluate(&doc, &Criterion::Contains { lhs: field("title"), element: val(json!("world")) }));
    }

    #[test]
    fn concat_operand_in_criterion() {
        let doc = json!({ "first": "hello", "last": "world" });
        let lhs = Operand::Concat(vec![field("first"), Operand::Value(json!("-")), field("last")]);
        assert!(evaluate(&doc, &Criterion::Eq { lhs, rhs: val(json!("hello-world")) }));
    }

    #[test]
    fn sum_operand_in_criterion() {
        let doc = json!({ "a": 3, "b": 4 });
        let lhs = Operand::Sum(vec![field("a"), field("b")]);
        assert!(evaluate(&doc, &Criterion::Gt { lhs, rhs: val(json!(6)) }));
    }
}
