# Document Filtering

The `document/filtering` module provides a composable, serializable query language for filtering JSON documents.
It is used by the [Catalog Server](catalog-server.md) `SearchResources` action and can be applied to any `serde_json::Value`.

## Concepts

There are three building blocks:

| Concept | Role |
|---|---|
| **`Query`** | Combinator tree — composes criteria with AND / OR / XOR / NOT, or wraps a single leaf criterion |
| **`Criterion`** | A comparison — has zero, one, or two `Operand` sides depending on the operator |
| **`Operand`** | A value source — a document field, a literal, or a computed value (concat, sum, …) |

Separating `Operand` from `Criterion` means both sides of any comparison can independently be a field reference, a literal, or a computation.

---

## Query

A `Query` is a JSON object with exactly one key.

```
Query =
  | { "all":    [ Query, … ] }   -- AND: every sub-query must match
  | { "any":    [ Query, … ] }   -- OR:  at least one must match
  | { "one_of": [ Query, … ] }   -- XOR: exactly one must match
  | { "not":    Query }          -- NOT: sub-query must not match
  | { "when":   Criterion }      -- leaf: a single criterion
```

Empty `all` is vacuously true (pass-through). Empty `any` is vacuously false.

---

## Operand

An `Operand` resolves to an optional JSON value when evaluated against a document.

| Variant | JSON | Resolves to |
|---|---|---|
| `Field` | `{ "field": "metadata.namespace" }` | value at dot-path, or absent |
| `Value` | `{ "value": "testing" }` | the literal; always present |
| `Concat` | `{ "concat": [ op, … ] }` | string concatenation; absent if any part is not a string |
| `Sum` | `{ "sum": [ op, … ] }` | `f64` sum; absent if any part is not numeric |

`Field` uses a **dot-separated path**: `"metadata.namespace"` traverses `doc["metadata"]["namespace"]`. Any absent segment makes the whole path absent.

---

## Criterion

A `Criterion` is a JSON object with exactly one key (the operator). Both sides of a binary comparison are `Operand`s.

### Equality

| Criterion | JSON form | Notes |
|---|---|---|
| `eq`  | `{ "eq":  { "lhs": <operand>, "rhs": <operand> } }` | True when both are absent |
| `neq` | `{ "neq": { "lhs": <operand>, "rhs": <operand> } }` | False when `lhs` is absent |

### Ordering

Numbers are compared as `f64`; strings lexicographically. Returns `false` for absent or type-mismatched operands.

| Criterion | JSON form |
|---|---|
| `lt`  | `{ "lt":  { "lhs": <operand>, "rhs": <operand> } }` |
| `lte` | `{ "lte": { "lhs": <operand>, "rhs": <operand> } }` |
| `gt`  | `{ "gt":  { "lhs": <operand>, "rhs": <operand> } }` |
| `gte` | `{ "gte": { "lhs": <operand>, "rhs": <operand> } }` |

### String / array membership

| Criterion | JSON form | `lhs` type | `element`/`pattern` type |
|---|---|---|---|
| `contains` | `{ "contains": { "lhs": <operand>, "element": <operand> } }` | array or string | any / substring |
| `regex`    | `{ "regex":    { "lhs": <operand>, "pattern": <operand> } }` | string | string regex pattern |

### Presence

| Criterion | JSON form | Notes |
|---|---|---|
| `exists`  | `{ "exists":  <operand> }` | Operand resolves to any value (including `null`) |
| `missing` | `{ "missing": <operand> }` | Operand does not resolve |

---

## Absent operand behaviour

- `eq` — true only when both sides are absent simultaneously.
- `neq` — false when `lhs` is absent.
- All ordering criteria — false for absent or type-incompatible operands.
- `contains`, `regex` — false when either operand is absent or the wrong type.
- `exists` / `missing` — presence test by definition.

---

## Examples

### Match by kind and namespace (field vs literal)

```json
{ "all": [
    { "when": { "eq": { "lhs": { "field": "kind" },               "rhs": { "value": "workaholic/Task:1.0" } } } },
    { "when": { "eq": { "lhs": { "field": "metadata.namespace" }, "rhs": { "value": "testing" } } } }
]}
```

### Tag or name pattern

```json
{ "any": [
    { "when": { "contains": { "lhs": { "field": "metadata.tags" }, "element": { "value": "build" } } } },
    { "when": { "regex":    { "lhs": { "field": "name" },          "pattern": { "value": "^cargo-.*" } } } }
]}
```

### Compound: namespace + (tag or name) + not deprecated

```json
{ "all": [
    { "when": { "eq": { "lhs": { "field": "metadata.namespace" }, "rhs": { "value": "testing" } } } },
    { "any": [
        { "when": { "contains": { "lhs": { "field": "metadata.tags" }, "element": { "value": "build" } } } },
        { "when": { "regex":    { "lhs": { "field": "name" },          "pattern": { "value": "^cargo-.*" } } } }
    ]},
    { "not": { "when": { "exists": { "field": "spec.deprecated" } } } }
]}
```

### Field vs field (cross-field comparison)

```json
{ "when": { "eq": { "lhs": { "field": "spec.owner" }, "rhs": { "field": "metadata.owner" } } } }
```

### Computed operand — concatenation

```json
{ "when": { "eq": {
    "lhs": { "concat": [ { "field": "kind" }, { "value": "/" }, { "field": "name" } ] },
    "rhs": { "value": "workaholic/Task:1.0/cargo-build" }
}}}
```

### Computed operand — numeric sum threshold

```json
{ "when": { "gt": {
    "lhs": { "sum": [ { "field": "spec.cpu" }, { "field": "spec.overhead" } ] },
    "rhs": { "value": 10 }
}}}
```

### All documents (pass-through)

```json
{ "all": [] }
```

---

## Extensibility

Adding a new operand kind (e.g. `lower`, `coalesce`):
1. Add a variant to `Operand` in [`expr.rs`](../rust/workaholic/crates/orkester-plugin-workaholic/src/document/filtering/expr.rs)
2. Add a `match` arm in [`operand.rs`](../rust/workaholic/crates/orkester-plugin-workaholic/src/document/filtering/operand.rs)

Adding a new criterion (e.g. `starts_with`, `date_before`):
1. Add a variant to `Criterion` in [`expr.rs`](../rust/workaholic/crates/orkester-plugin-workaholic/src/document/filtering/expr.rs)
2. Add a `match` arm in [`compare.rs`](../rust/workaholic/crates/orkester-plugin-workaholic/src/document/filtering/compare.rs)

The `rename_all = "snake_case"` attribute on all enums derives JSON keys automatically. The round-trip test in `expr.rs` enforces that the serialization contract stays stable.

---

## Source layout

```
document/filtering/
├── mod.rs       pub mod + pub use only
├── expr.rs      Query / Criterion / Operand expression tree  (serialization contract + tests)
├── field.rs     dot-path resolution into serde_json::Value
├── operand.rs   Operand resolution: field, literal, concat, sum
├── compare.rs   Criterion evaluation against a document
└── eval.rs      recursive apply() + filter() + integration tests
```
