# Catalog Server

The Catalog Server is an in-memory, namespace-aware resource store.
It is fed by document loaders (local filesystem, S3, etc.) and queried by the workflow engine and the REST API.

## Component kind

```
workaholic/CatalogServer:1.0
```

## Purpose

- Index every document emitted by loaders (Tasks, Works, Namespaces, Profiles, …)
- Provide CRUD operations on individual resources by ID
- Expose a composable search API for the REST layer

Resources are stored as raw `serde_json::Value` and keyed by their canonical ID:

```
{kind}/{namespace}/{name}/{version}
```

For example: `workaholic/Task:1.0/testing/cargo-build/1.0.0`

## Actions

| Action kind | Input | Output |
|---|---|---|
| `workaholic/CatalogServer/CreateResource` | `{ id, resource }` | stored resource |
| `workaholic/CatalogServer/RetrieveResource` | `{ id }` | resource or `NotFound` |
| `workaholic/CatalogServer/UpdateResource` | `{ id, resource }` | updated resource or `NotFound` |
| `workaholic/CatalogServer/DeleteResource` | `{ id }` | `true` or `NotFound` |
| `workaholic/CatalogServer/SearchResources` | `{ query: Query }` | `[ resource, … ]` |
| `workaholic/CatalogServer/ListNamespaces` | `{}` | `{ namespaces: [ … ] }` |
| `workaholic/CatalogServer/ListWorks` | `{ ns }` | `{ works: [ … ] }` |
| `workaholic/CatalogServer/ListTasks` | `{ ns }` | `{ tasks: [ … ] }` |

`CreateResource` is an upsert — it overwrites any existing resource with the same ID.

## Document loader events

The Catalog Server subscribes to loader change events:

| Event | Effect |
|---|---|
| `DocumentAdded` | Upserts the document by canonical ID |
| `DocumentModified` | Overwrites the previous version |
| `DocumentRemoved` | Deletes the document by canonical ID |

## Configuration

```yaml
- name: catalog-server
  kind: workaholic/CatalogServer:1.0
  config: {}
```

No configuration options are currently required. Future options (persistence backend, indexing, access control) will be added here.

## Search

See [document-filtering.md](document-filtering.md) for the full `SearchResources` query language reference.

Quick example — find all Tasks in namespace `testing` tagged `build`:

```json
{
  "query": {
    "all": [
      { "when": { "eq": { "lhs": { "field": "kind" },               "rhs": { "value": "workaholic/Task:1.0" } } } },
      { "when": { "eq": { "lhs": { "field": "metadata.namespace" }, "rhs": { "value": "testing" } } } },
      { "when": { "contains": { "lhs": { "field": "metadata.tags" }, "element": { "value": "build" } } } }
    ]
  }
}
```
