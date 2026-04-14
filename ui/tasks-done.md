### Task 005 — Catalog list page
**File**: `src/pages/catalog/index.js`

**Document identity**: a document is uniquely identified by **(kind, namespace, name, version)**. Namespace may be absent/empty for global resources (e.g. `workaholic/Namespace:1.0` documents). The server stores `ns = "global"` when namespace is null. Multiple versions of the same (kind, ns, name) can coexist.

**Requirements**:
- Call `searchDocuments(query)` from `../api.js`. Start with an empty query `{}` to fetch all documents visible to the active namespace (the backend filters by namespace). Alternatively pass `{ namespace: getActiveNamespace() }` if the query supports it.
- **Group results client-side** by `(kind, namespace, name)` — each row in the table represents one logical document, not one version. For each group, track: `versions` (sorted array of version strings, latest first), `latest` (the doc with the highest version), `count` (number of versions).
- Toolbar: search input (`q`, filters kind + name + description), kind filter dropdown (populated from unique kinds in all results), namespace filter (pre-populated with active namespace, with "global" option).
- **Table columns**: Kind, Namespace (show `—` for global), Name, Versions (`count` bubble + latest version string, e.g. `3 · v1.2`), Description (`metadata.description` from latest), Owner (`metadata.owner` from latest).
- Clicking a row navigates to `#/catalog/detail?kind=${encodeURIComponent(kind)}&ns=${encodeURIComponent(ns)}&name=${encodeURIComponent(name)}` — no version in URL (the detail page shows all versions).
- Breadcrumb: Catalog
- Page state: `{ q, kindFilter, nsFilter, sortKey, sortDir, page }` — local variables, not persisted.
- Re-fetch on namespace change: `window.addEventListener('orkester:namespace-changed', () => navigate('#/catalog'))`

---

### Task 006 — Catalog document detail page
**File**: `src/pages/catalog/detail.js`  
**Route**: `#/catalog/detail?kind=...&ns=...&name=...` (version is optional; if absent, show latest)

**Editor library**: Use **CodeMirror 6** for inline editing. It must be loaded in `index.html` (add before `<script type="module" src="src/app.js">`):  
```html
<!-- CodeMirror 6 - JSON editor (uses stable @6 scoped packages, NOT the codemirror@6 npm package which is the old CM5 API) -->
<script type="importmap">
{
  "imports": {
    "@codemirror/view":     "https://esm.sh/@codemirror/view@6",
    "@codemirror/state":    "https://esm.sh/@codemirror/state@6",
    "@codemirror/language": "https://esm.sh/@codemirror/language@6",
    "@codemirror/lang-json": "https://esm.sh/@codemirror/lang-json@6"
  }
}
</script>
```
Then in the page JS:
```js
import { EditorView, lineNumbers, highlightActiveLine }  from '@codemirror/view'
import { syntaxHighlighting, defaultHighlightStyle }      from '@codemirror/language'
import { json }                                           from '@codemirror/lang-json'

// Minimal editor (no basicSetup — @codemirror/basic-setup@0.20.0 uses incompatible 0.x deps)
const extensions = [json(), lineNumbers(), highlightActiveLine(), syntaxHighlighting(defaultHighlightStyle)]
new EditorView({ doc: value, extensions, parent: element })
// Read-only: add EditorView.editable.of(false) to extensions
```
Destroy the editor instance in `setCleanup` to avoid leaks.

**Requirements**:
- Read `kind`, `ns`, `name` from `query`. `version` is optional — if absent, default to the latest.
- Call `searchDocuments({ kind, namespace: ns === 'global' ? null : ns, name })` to fetch **all versions** of this document. Sort versions (semantic or lexicographic, descending).
- **Version selector**: a `<select>` dropdown (or a row of version tabs if ≤ 5 versions) at the top of the page. Switching version re-renders the spec/status panels without a page reload. The active version is tracked in local state: `let activeVersion = latestVersion`.
- **Layout** (no modals — everything is inline):
  - Page title: `name` + kind chip (plain `<code>` label, not a coloured badge)
  - Metadata row: namespace, owner, description, tags
  - Version selector (see above)
  - Two-column tab bar below the selector: **Spec** | **Status** | **Raw** (Raw = full document JSON)
  - **Spec tab** (default): renders `spec` via a 
    - **View mode**: `<pre class="code-block">` with formatted JSON
    - **Edit mode** (toggled by an Edit button): replaces the `<pre>` with a CodeMirror instance. The editor is initialised with `JSON.stringify(doc.spec, null, 2)`. Toolbar: **Save** (calls `updateDocument(id, updatedDoc)`, shows `toastSuccess`, exits edit mode) and **Cancel** (discards changes, exits edit mode).
  - **Status tab**: `<pre class="code-block">` read-only — status is never editable.
  - **Raw tab**: full document JSON in a read-only CodeMirror instance (for easy copy).
- **Actions** (page-level, top-right):
  - **New version** button: navigates to `#/catalog/new?kind=...&ns=...&name=...` (Task 006b, stubbed for now)
  - **Delete version** button: confirm → `deleteDocument(id)` → if more versions remain, reload; if last version, navigate back to `#/catalog`
- **Document ID construction**: `id = kind + '/' + (ns || 'global') + '/' + name + '/' + version`
- Breadcrumb: Catalog → kind → `name`

**CodeMirror setup snippet** (for reference):
```js
function createEditor(containerId, value, readonly = false) {
  const extensions = [basicSetup, json()]
  if (readonly) extensions.push(EditorView.editable.of(false))
  return new EditorView({
    doc: value,
    extensions,
    parent: document.getElementById(containerId),
  })
}
function getEditorValue(view) { return view.state.doc.toString() }
function destroyEditor(view) { if (view) view.destroy() }
```

---

