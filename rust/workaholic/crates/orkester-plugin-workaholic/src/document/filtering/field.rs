//! JSON field path resolution.

use serde_json::Value;

/// Resolve a dot-separated path into a JSON value.
///
/// `"metadata.namespace"` traverses `doc["metadata"]["namespace"]`.
/// Returns `None` if any segment is absent or a parent is not an object.
pub fn resolve<'a>(doc: &'a Value, path: &str) -> Option<&'a Value> {
    path.split('.').try_fold(doc, |node, seg| node.get(seg))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn resolves_nested_path() {
        let v = json!({ "metadata": { "namespace": "testing" } });
        assert_eq!(resolve(&v, "metadata.namespace"), Some(&json!("testing")));
    }

    #[test]
    fn top_level_field() {
        let v = json!({ "kind": "workaholic/Task:1.0" });
        assert_eq!(resolve(&v, "kind"), Some(&json!("workaholic/Task:1.0")));
    }

    #[test]
    fn missing_segment_returns_none() {
        let v = json!({ "metadata": {} });
        assert_eq!(resolve(&v, "metadata.namespace"), None);
    }

    #[test]
    fn non_object_parent_returns_none() {
        // "tags" is an array, not an object — further traversal must return None.
        let v = json!({ "tags": ["a", "b"] });
        assert_eq!(resolve(&v, "tags.foo"), None);
    }

    #[test]
    fn deeply_nested_path() {
        let v = json!({ "a": { "b": { "c": 42 } } });
        assert_eq!(resolve(&v, "a.b.c"), Some(&json!(42)));
    }
}
