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

### Task 015 — Settings page
**File**: `src/pages/settings.js`

**Requirements**:
- Two sections via sub-nav tabs:
  1. **User Settings** — Settings are store in the catalog as `core/UserSettings:1.0` (global, no namespace), with the name in the form `<user_id>-settings`. The spec is a list of properties with `key`, `value`, and `type` (`string`, `number`, `boolean`, ...). The page renders a form based on the spec, allowing users to edit and save their preferences. On save, upsert the document via `createDocument`.
  2. **Application Settings** — Settings are stored in the catalog as `core/AppSettings:1.0` (global, no namespace). The spec is a list of config entries with `key`, `value`, and `type`. The page renders a form based on the spec, allowing users to edit and save the configuration. On save, upsert the document via `createDocument`.
- User ID can be obtained from `getActiveUser().id` from `../../state.js` (mocked for now)
- Authorization is out of scope for this task — assume all users can read/write the settings.
- Breadcrumb: Settings
- Create the `core/UserSettings:1.0` and `core/AppSettings:1.0` documents in the backend (folder `dev/catalog/core`) with some sample properties for testing.

---

### Task 016 — Help page
**File**: `src/pages/help.js`

**Requirements**:
- Call `searchDocuments(...)` to list all `core/HelpTopic:1.0` documents in the current and global namespaces
- Structure of `core/HelpTopic:1.0`:
  ```
  {
    kind: "core/HelpTopic:1.0",
    name, version,
    metadata: { namespace, description },
    spec: { title, content, listable }
  }
  ```
- If no help topics exist, show a friendly empty state
- The main page shows a list of help topics where `spec.listable === true` as cards with `spec.title` and `metadata.description`
- Clicking a card navigates to `#/help/${name}` and shows the full `spec.content` rendered (Markdown → HTML, use a library like [marked](https://github.com/markedjs/marked)).
- Ensure a topic can link to other topics via Markdown links to `#/help/${name}`
- Breadcrumb: Help > `spec.title`
- Create a few sample `core/HelpTopic:1.0` documents in the backend (folder `dev/catalog/help`) for testing (e.g. "Getting Started", "FAQ", "API Reference")