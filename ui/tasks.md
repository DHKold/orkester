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
