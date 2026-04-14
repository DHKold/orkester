# Orkester UI — Implementation Tasks

> **How to use this file**: Each task is self-contained. Read the **Context** and **Data models** sections before implementing. All files use ES modules (no bundler). No framework — vanilla JS + DOM manipulation. CSS classes are defined in `styles/components.css`.

---

## Foundation (already implemented)

The following infrastructure is already in place — do **not** re-implement:

| File | Purpose |
|------|---------|
| `index.html` | Entry point, CDN scripts (Cytoscape, Chart.js, **CodeMirror 6**), sidebar HTML shell |
| `styles/variables.css` | CSS custom properties (colours, spacing, radii) |
| `styles/reset.css` | Base reset |
| `styles/layout.css` | Sidebar + page-wrap grid layout, collapse/resize |
| `styles/components.css` | All shared component styles |
| `src/router.js` | Hash router (`route()`, `navigate()`, `setCleanup()`, `start()`) |
| `src/api.js` | All HTTP calls — catalog: `searchDocuments(query)`, `getDocument(id)`, `createDocument(body)`, `updateDocument(id, body)`, `deleteDocument(id)`. Workflow: `listWorkRuns()`, `getWorkRun()`, `listCrons()`, etc. Document ID format: `{kind}/{ns}/{name}/{version}` where `ns` defaults to `global` for namespace-less documents. |
| `src/utils.js` | `esc()`, `fmtDate()`, `fmtDuration()`, `setApp()`, `setBreadcrumb()`, `applyFilter()`, `applySort()`, `paginate()`, `isTerminal()` |
| `src/state.js` | `getActiveNamespace()`, `setActiveNamespace()`, `MOCK_USER` |
| `src/components/sidebar.js` | Full sidebar (user, namespace switcher, nav, collapse, resize) |
| `src/components/toast.js` | `toast()`, `toastSuccess()`, `toastError()` |
| `src/components/modal.js` | `openModal()`, `closeModal()`, `setModalBody()` |
| `src/components/badge.js` | `badge(status)` |
| `src/components/table.js` | `renderTable(opts)`, `bindTable(id, handlers)` |
| `src/components/kv-editor.js` | `renderKvEditor()`, `bindKvEditor()`, `readKvEditor()` |
| `src/components/dag.js` | `renderDag()`, `updateDagColors()`, `destroyDag()` |
| `src/components/log-viewer.js` | `renderLogViewer()`, `bindLogViewer()`, `renderStructuredLogs()` |
| `src/app.js` | Route registrations + `initSidebar()` + `start()` |

---

## Key data models (backend reference)

<details>
<summary>WorkRun — runtime execution of a Work DAG</summary>

```
{
  name, kind: "workaholic/WorkRun:1.0", version, metadata: { namespace },
  spec: { workRef, workRunnerRef, trigger: { kind: "manual"|"cron", ... } },
  status: {
    state: "pending"|"running"|"succeeded"|"failed"|"cancelled",
    createdAt, startedAt, finishedAt,
    steps: [{ name, state, taskRunRequestRef, attempts }],
    summary: { totalSteps, pendingSteps, runningSteps, succeededSteps, failedSteps, cancelledSteps },
    outputs, logs: [{ timestamp, level, message }], stateHistory
  }
}
```
</details>

<details>
<summary>TaskRun — execution of a single step</summary>

```
{
  name, kind: "workaholic/TaskRun:1.0",
  spec: { taskRef, workRunRef, stepName, attempt, taskRunnerRef },
  status: {
    state, createdAt, startedAt, finishedAt,
    inputs, outputs,
    logsRef: { stdout: "<uri>", stderr: "<uri>" },
    stateHistory
  }
}
```
</details>

<details>
<summary>WorkRunner — orchestrates WorkRuns</summary>

```
{
  name, kind: "workaholic/WorkRunner:1.0",
  spec: { kind: "workaholic/ThreadWorkRunner:1.0", concurrency: { max_work_runs, max_task_runs }, labels },
  status: {
    state: "creating"|"active"|"inactive"|"dropped",
    active_work_runs, active_task_runs,
    stateHistory
  }
}
```
</details>

<details>
<summary>TaskRunner — executes individual steps</summary>

```
{
  name, kind: "workaholic/TaskRunner:1.0",
  spec: { kind: "workaholic/ShellTaskRunner:1.0"|"...ContainerTaskRunner..."|"...KubernetesTaskRunner..."|"...HttpTaskRunner:1.0", config },
  status: { state: "ready"|"running"|"dropped", metrics: { total_time_seconds }, stateHistory }
}
```
</details>

<details>
<summary>Cron — scheduled Work execution</summary>

```
{
  name, kind: "workaholic/Cron:1.0",
  spec: {
    enabled: bool, work_ref, schedules: ["0 */6 * * *"], timezone,
    validity: { start, end, max_runs },
    params: { key: value }, concurrency: "allow"|"skip"|"replace"|"wait"
  },
  status: {
    last_scheduled_time, next_scheduled_time,
    last_run_status, consecutive_failures, run_count
  }
}
```
</details>

<details>
<summary>Catalog document (generic)</summary>

```
{
  kind: "<group>/<Kind>:<major.minor>",  // e.g. "workaholic/Work:1.0"
  name, version,
  metadata: { namespace, owner, description, tags: string[] },
  spec: { ... },   // shape depends on kind
  status: { ... }
}
```
</details>

---

## Tasks

### Task 004 — Dashboard page
**File**: `src/pages/dashboard.js`  
**Stub**: already exists, replaces the placeholder `setApp()` call.

**Requirements**:
- Widget grid using CSS Grid (`display: grid; grid-template-columns: repeat(auto-fill, minmax(300px, 1fr))`)
- Initial widgets (hardcoded):
  1. **Recent Work Runs** — table of last 5 WorkRuns from `listWorkRuns(ns)` with name, status badge, created time
  2. **Active Runners** — count cards for WorkRunners in state `active` vs total from `listWorkRunners(ns)`
  3. **Cron Overview** — count of enabled/disabled crons from `listCrons(ns)`
  4. **System Health** — uptime + version from `getHealth()`
- Widget layout (order + visibility) persisted in `localStorage` under key `orkester-dashboard-layout`
- "Customize" button opens a panel to reorder/toggle widgets
- Each widget has a title bar and loads its data independently (individual try/catch, shows error state per widget)
- Import: `getActiveNamespace` from `../../state.js`

---

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

### Task 007 — Workspace: Runners page
**File**: `src/pages/workspace/runners.js`

**Requirements**:
- Two tabs using `.tabs` / `.tab-btn`: **Work Runners** and **Task Runners**
- **Work Runners tab**:
  - Call `listWorkRunners(ns)` from `../../api.js`
  - Table columns: Name, State (badge), Kind (`spec.kind`), Active Work Runs, Active Task Runs, Max Work Runs (`spec.concurrency.max_work_runs`)
  - "Active Work Runs" cell: render as a load gauge (`renderLoadGauge(active, max)`) — see Load gauge helper below
  - Row click → `navigate('#/workspace/runners/' + encodeURIComponent(row.name))`
  - Global action button: **New WorkRunner** → opens modal with form (name, kind dropdown: `workaholic/ThreadWorkRunner:1.0`, thread count, max_work_runs, max_task_runs)
- **Task Runners tab**:
  - Call `listTaskRunners(ns)`
  - Table columns: Name, Kind (`spec.kind`), State (badge), Total time (`status.metrics.total_time_seconds` formatted)
  - Row click → `navigate('#/workspace/task-runners/' + encodeURIComponent(row.name))`
  - No create action (read-only)
- Active tab persisted in `sessionStorage` key `orkester-runners-tab`

**Load gauge helper** (add as local function):
```js
function renderLoadGauge(active, max) {
  if (max == null || max === 0) return `${active ?? 0}`
  const pct   = Math.min(100, Math.round((active / max) * 100))
  const cls   = pct > 80 ? 'high' : pct > 50 ? 'medium' : ''
  return `<div class="load-gauge">
    <div class="load-bar-track"><div class="load-bar-fill ${cls}" style="width:${pct}%"></div></div>
    <span class="load-label">${active}/${max}</span>
  </div>`
}
```

---

### Task 008 — Workspace: WorkRunner detail page
**File**: `src/pages/workspace/runner-detail.js`

**Requirements**:
- Call `getWorkRunner(name)` from `../../api.js`
- Display:
  - Page title: runner `name` + state badge
  - **Live load gauges** (prominent, top of page) for `active_work_runs` / `max_work_runs` and `active_task_runs` / `max_task_runs`
  - Detail grid: kind, created/updated timestamps, labels
  - **Spec section**: concurrency config as detail fields
  - **State history** table: timestamp, from-state, to-state
  - **Work Runs** section: call `listWorkRuns(ns)`, filter by `spec.workRunnerRef === name`, render as table (name, work ref, status badge, created time) with row click to work run detail
- Actions: Delete WorkRunner (confirm + `deleteWorkRunner(name)` + navigate back)
- Breadcrumb: Workspace → Runners → `name`
- Auto-refresh every 5 s (poll `getWorkRunner`), stop when state is `dropped`

---

### Task 009 — Workspace: TaskRunner detail page
**File**: `src/pages/workspace/task-runner-detail.js`

**Requirements**:
- Call `getTaskRunner(name)` from `../../api.js`
- Display:
  - Page title: runner `name` + state badge
  - Detail grid: kind, state, total_time_seconds (formatted as duration)
  - **State history** table: timestamp, state
  - **Spec** as code block (JSON)
- No create/delete actions (read-only)
- Breadcrumb: Workspace → Runners → `name`

---

### Task 010 — Workspace: Work Runs list page
**File**: `src/pages/workspace/work-runs.js`

**Requirements**:
- Call `listWorkRuns(ns)` from `../../api.js`
- Table columns: Name, Work (`spec.workRef` — show short name only, full ref in title attr), Status (badge), Trigger (`spec.trigger.kind`), Created (`status.createdAt` — relative), Duration (`fmtDuration(startedAt, finishedAt)`), Steps progress (`${succeeded}/${total}`)
- Sort default: `status.createdAt` descending
- Search filters: free text `q` (filters name + workRef), status filter dropdown
- Cancel button on non-terminal rows: `cancelWorkRun(name)`, show `toastSuccess`
- Row click → `navigate('#/workspace/work-runs/' + encodeURIComponent(row.name))`
- Global action: **New Work Run** modal:
  - Work selector: `select` populated from `listWorks(ns)` grouped by namespace
  - KV params editor (use `renderKvEditor` / `bindKvEditor` / `readKvEditor`)
  - WorkRunner selector: `select` from `listWorkRunners(ns)` (optional)
  - Submit calls `triggerWork({ workRef, inputs, workRunnerRef })`, then navigates to the new run
- Auto-refresh: 30 s countdown with pause toggle + manual refresh button
- Breadcrumb: Workspace → Work Runs

---

### Task 011 — Workspace: Work Run detail page
**File**: `src/pages/workspace/work-run-detail.js`

**Requirements**:
- Call `getWorkRun(name)` from `../../api.js`
- Sections (in order):
  1. **Header card**: work ref, status badge, trigger kind, created/started/finished times, duration, runner ref
  2. **Stats grid** (5 cards): Total Steps, Pending, Running, Succeeded, Failed
  3. **DAG visualization**: call `renderDag('dag-container', run.status.steps, workSpec)` — fetch `workSpec` via `getWork(ns, workName, version)` parsed from `run.spec.workRef`. On step click, scroll to and expand that step card. Wire reset button: `<button class="btn btn-secondary btn-sm" data-dag-reset>Reset view</button>`
  4. **Step filter bar**: All / Running / Failed / Succeeded buttons (filter `.step-card` visibility)
  5. **Steps list**: one `.step-card` per step — header shows step name, state badge, attempt count. Expanding a step card:
     - Calls `getTaskRun(step.taskRunRequestRef)` (cache by name to avoid re-fetching)
     - Renders task run detail: inputs table, outputs table
     - Renders log viewer: `renderLogViewer('log-' + stepName, taskRun.status.logsRef)` then `bindLogViewer(...)`
  6. **Run logs**: `renderStructuredLogs(run.status.logs)`
- Auto-refresh every 3 s while not terminal: updates header status, stats, step states, DAG colors (`updateDagColors`). Stop polling when `isTerminal(run.status.state)`.
- Cancel button (non-terminal only): `cancelWorkRun(name)`
- Call `setCleanup(() => destroyDag('dag-container'))` in router cleanup
- Breadcrumb: Workspace → Work Runs → `name`

---

### Task 012 — Workspace: Crons list page
**File**: `src/pages/workspace/crons.js`

**Requirements**:
- Call `listCrons(ns)` from `../../api.js`
- Table columns: Name, Schedules (`spec.schedules.join(', ')`), Work (`spec.work_ref` short name), Enabled (badge: enabled/disabled), Next Run (`status.next_scheduled_time` relative), Run Count (`status.run_count`), Consecutive Failures
- Row click → `navigate('#/workspace/crons/' + encodeURIComponent(row.name))`
- Row actions (action column, not row click):
  - **Toggle** enable/disable: `registerCron({ ...cron, spec: { ...cron.spec, enabled: !cron.spec.enabled } })`
  - **Delete**: confirm + `unregisterCron(name)` + refresh
- Global action: **New Cron** → opens create/edit modal (see modal spec below)
- Breadcrumb: Workspace → Crons

**Cron create/edit modal fields**:
- Name (text, readonly on edit)
- Work ref (text input with datalist from `listWorks(ns)`)
- Schedules (text, comma-separated cron expressions, e.g. `0 */6 * * *`)
- Timezone (text, default `UTC`)
- Enabled checkbox (default true)
- Concurrency (select: allow / skip / replace / wait)
- Params (KV editor via `renderKvEditor` / `bindKvEditor` / `readKvEditor`)
- Submit calls `registerCron(doc)` — build a full `workaholic/Cron:1.0` doc

---

### Task 013 — Workspace: Cron detail page
**File**: `src/pages/workspace/cron-detail.js`

**Requirements**:
- Call `listCrons(ns)` + filter by name, or add `getCron(name)` to `api.js` if the endpoint exists
- Display:
  - Page title: cron `name` + enabled badge
  - **Status cards** (top): Last Status (badge), Next Run (relative + absolute), Last Run (relative), Run Count, Consecutive Failures
  - Detail grid: work ref, schedules, timezone, concurrency policy, validity (start/end/max_runs)
  - **Params** section: render `spec.params` as a key-value table
  - **Triggered Work Runs**: call `listWorkRuns(ns)`, filter where `spec.trigger.kind === 'cron'` and trigger matches cron name, render as table
- Actions: Edit (opens same modal as Task 012 with pre-filled values), Toggle enabled, Delete
- Breadcrumb: Workspace → Crons → `name`

---

### Task 014 — Metrics page
**File**: `src/pages/metrics.js`

**Requirements**:
- Call `getMetricsSnapshot()` and `getMetricsHistory()` from `../../api.js`
- **Current values grid**: cards showing metric name → current value (number formatted with `toLocaleString`)
- **History section**: for each metric with ≥ 2 history points, render a chart card:
  - Card title: metric name (abbreviated: last segment after `/` or `.`)
  - Trend indicator: ▲ (last > prev), ▼ (last < prev), — (equal) with colour
  - Chart.js line chart (100 × 48 px sparkline canvas): `new Chart(canvas, { type:'line', data: {...}, options: { responsive:false, plugins:{legend:{display:false}}, scales:{x:{display:false},y:{display:false}} } })`
- History cards in a responsive grid (3–4 cols)
- Auto-refresh every 30 s; destroy and recreate Chart instances on each refresh to avoid leaks (use `setCleanup`)
- Breadcrumb: Metrics

---

### Task 015 — Settings page
**File**: `src/pages/settings.js`

**Requirements**:
- Two sections via sub-nav tabs:
  1. **User Preferences** — load/save a `core/UserSettings:1.0` document via `searchDocuments({ kind: 'core/UserSettings:1.0', namespace: ns })` / `createDocument` / `updateDocument`. Display as a simple form with a JSON textarea (or CodeMirror editor if already loaded) for the `spec` object.
  2. **App Configuration** — same pattern for `core/AppConfig:1.0`
- If no document exists yet, show an "Initialize" button that calls `createDocument()` with an empty spec (`{ kind: 'core/UserSettings:1.0', name: 'default', version: '1.0', metadata: { namespace: ns }, spec: {} }`)
- Breadcrumb: Settings

---

### Task 016 — Help page
**File**: `src/pages/help.js`

**Requirements**:
- Call `searchDocuments({ kind: 'core/HelpTopic:1.0', namespace: getActiveNamespace() })`
- If no help topics exist, show a friendly empty state with a link to the README
- List topics as clickable cards (title from `name` or `metadata.description`, body from `spec.content`)
- Clicking a card expands it in-place (toggle `.open` class) to show `spec.content` rendered as preformatted text or markdown (use `<pre>` for now, optionally a simple markdown renderer)
- Search input to filter topics by title/content
- Breadcrumb: Help

---

## Implementation notes

- **Namespace scoping**: always read `getActiveNamespace()` at render-time (not at module load). All list API calls accept a `ns` parameter.
- **Auto-refresh pattern**:
  ```js
  let interval = null
  function refresh() { /* fetch + re-render */ }
  refresh()
  interval = setInterval(refresh, 30_000)
  setCleanup(() => clearInterval(interval))
  ```
- **Polling until terminal** (Task 011):
  ```js
  async function poll() {
    const run = await getWorkRun(name)
    // update UI ...
    if (isTerminal(run.status.state)) clearInterval(interval)
  }
  interval = setInterval(poll, 3_000)
  setCleanup(() => { clearInterval(interval); destroyDag('dag-container') })
  ```
- **Error handling**: wrap all API calls in try/catch; show `toastError(err.message)` and render an error state in the relevant section.
- **Table pattern**: use `renderTable` + `bindTable` from `src/components/table.js`. Keep a local `state` object with `{ q, sortKey, sortDir, page }`. Call a `render()` function that applies filter/sort/paginate then re-renders with `document.getElementById('table-wrap').innerHTML = renderTable(...)` and `bindTable('table-wrap', ...)`.
- **Navigate**: import `{ navigate }` from `../../router.js` or `../router.js` as appropriate.
