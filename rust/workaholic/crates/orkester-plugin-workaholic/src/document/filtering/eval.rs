//! Query tree evaluation over JSON documents.

use serde_json::Value;

use super::compare::evaluate as eval_criterion;
use super::expr::Query;

/// Returns `true` if `doc` satisfies `query`.
pub fn apply(query: &Query, doc: &Value) -> bool {
    match query {
        Query::All(qs)   => qs.iter().all(|q| apply(q, doc)),
        Query::Any(qs)   => qs.iter().any(|q| apply(q, doc)),
        Query::Not(q)    => !apply(q, doc),
        Query::OneOf(qs) => qs.iter().filter(|q| apply(q, doc)).count() == 1,
        Query::When(c)   => eval_criterion(doc, c),
    }
}

/// Return only the documents from `docs` that satisfy `query`.
pub fn filter<'a>(query: &Query, docs: &'a [Value]) -> Vec<&'a Value> {
    docs.iter().filter(|doc| apply(query, doc)).collect()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::document::filtering::expr::{Criterion, Operand};

    use super::*;

    fn task(ns: &str, name: &str, tags: &[&str]) -> Value {
        json!({
            "kind": "workaholic/Task:1.0",
            "name": name,
            "metadata": { "namespace": ns, "tags": tags }
        })
    }

    /// Shorthand: `field == literal` leaf.
    fn eq(path: &str, v: serde_json::Value) -> Query {
        Query::When(Criterion::Eq { lhs: Operand::Field(path.into()), rhs: Operand::Value(v) })
    }

    #[test]
    fn all_requires_every_criterion() {
        let doc = task("ns-a", "t1", &[]);
        assert!( apply(&Query::All(vec![eq("metadata.namespace", json!("ns-a")), eq("name", json!("t1"))]), &doc));
        assert!(!apply(&Query::All(vec![eq("metadata.namespace", json!("ns-a")), eq("name", json!("x"))]),  &doc));
    }

    #[test]
    fn any_requires_at_least_one() {
        let doc = task("ns-a", "t1", &[]);
        assert!( apply(&Query::Any(vec![eq("name", json!("t1")), eq("name", json!("t99"))]), &doc));
        assert!(!apply(&Query::Any(vec![eq("name", json!("t2")), eq("name", json!("t99"))]), &doc));
    }

    #[test]
    fn not_inverts() {
        let doc = task("ns-a", "t1", &[]);
        assert!( apply(&Query::Not(Box::new(eq("name", json!("other")))), &doc));
        assert!(!apply(&Query::Not(Box::new(eq("name", json!("t1")))),    &doc));
    }

    #[test]
    fn one_of_is_exclusive_or() {
        let doc = task("ns-a", "t1", &[]);
        // Both match → false
        assert!(!apply(&Query::OneOf(vec![eq("name", json!("t1")), eq("metadata.namespace", json!("ns-a"))]), &doc));
        // Exactly one → true
        assert!( apply(&Query::OneOf(vec![eq("name", json!("t1")), eq("name", json!("other"))]), &doc));
        // None → false
        assert!(!apply(&Query::OneOf(vec![eq("name", json!("x")), eq("name", json!("y"))]), &doc));
    }

    #[test]
    fn filter_returns_matching_subset() {
        let docs = vec![task("ns-a", "t1", &[]), task("ns-a", "t2", &[]), task("ns-b", "t3", &[])];
        assert_eq!(filter(&eq("metadata.namespace", json!("ns-a")), &docs).len(), 2);
    }

    #[test]
    fn contains_tag_in_array() {
        let doc = task("ns-a", "t1", &["build", "rust"]);
        let contains = |el: &str| Query::When(Criterion::Contains {
            lhs:     Operand::Field("metadata.tags".into()),
            element: Operand::Value(json!(el)),
        });
        assert!( apply(&contains("rust"),   &doc));
        assert!(!apply(&contains("deploy"), &doc));
    }

    #[test]
    fn nested_all_and_any() {
        let doc = task("ns-a", "t1", &[]);
        let q = Query::All(vec![
            eq("metadata.namespace", json!("ns-a")),
            Query::Any(vec![eq("name", json!("t1")), eq("name", json!("t99"))]),
        ]);
        assert!(apply(&q, &doc));
    }

    #[test]
    fn field_missing_in_doc() {
        let doc = task("ns-a", "t1", &[]);
        assert!( apply(&Query::When(Criterion::Missing(Operand::Field("spec.runner".into()))), &doc));
        assert!(!apply(&Query::When(Criterion::Exists(Operand::Field("spec.runner".into()))),  &doc));
    }

    #[test]
    fn cross_field_comparison() {
        let doc = json!({ "spec": { "owner": "alice" }, "metadata": { "owner": "alice" } });
        let q = Query::When(Criterion::Eq {
            lhs: Operand::Field("spec.owner".into()),
            rhs: Operand::Field("metadata.owner".into()),
        });
        assert!(apply(&q, &doc));
    }

    #[test]
    fn concat_operand_in_query() {
        let doc = json!({ "kind": "Task", "name": "build" });
        let lhs = Operand::Concat(vec![
            Operand::Field("kind".into()),
            Operand::Value(json!("/")),
            Operand::Field("name".into()),
        ]);
        let q = Query::When(Criterion::Eq { lhs, rhs: Operand::Value(json!("Task/build")) });
        assert!(apply(&q, &doc));
    }
}
