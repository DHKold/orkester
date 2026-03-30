// DAG visualization using Cytoscape.js + dagre layout.
// Cytoscape and dagre are loaded as CDN <script> tags and available as globals.

const STATUS_COLORS = {
  pending:   { bg: '#94a3b8', text: '#1e293b' },
  running:   { bg: '#3b82f6', text: '#fff' },
  succeeded: { bg: '#22c55e', text: '#fff' },
  failed:    { bg: '#ef4444', text: '#fff' },
  skipped:   { bg: '#f59e0b', text: '#1e293b' },
  cancelled: { bg: '#6b7280', text: '#fff' },
}

/**
 * Render a DAG inside `container`.
 *
 * @param {HTMLElement}             container   - DOM element to render into
 * @param {object}                  work        - Work object from API (spec.steps, dependsOn)
 * @param {Record<string,object>}   stepStates  - Workflow.steps (step id → StepState)
 * @param {(stepId: string) => void} onNodeClick - called when a step node is clicked
 * @returns {object|null} Cytoscape instance, or null if no steps
 */
export function renderDag(container, work, stepStates, onNodeClick) {
  container.innerHTML = ''

  const steps = work?.spec?.steps ?? []
  if (steps.length === 0) {
    container.innerHTML = '<p style="text-align:center;padding:2rem;color:#94a3b8">No steps defined</p>'
    return null
  }

  // Build nodes
  const nodes = steps.map(step => {
    const stepId = step.name ?? step.id
    const state  = stepStates[stepId]
    const status = state?.status ?? 'pending'
    const colors = STATUS_COLORS[status] ?? STATUS_COLORS.pending
    return {
      data: {
        id:     stepId,
        label:  stepId,
        task:   step.task ?? '',
        status,
        bg:     colors.bg,
        text:   colors.text,
      }
    }
  })

  // Build edges — WorkStep.dependsOn (camelCase from Rust rename_all = "camelCase")
  const edges = []
  for (const step of steps) {
    const stepId = step.name ?? step.id
    for (const dep of step.dependsOn ?? step.depends_on ?? []) {
      edges.push({ data: { id: `${dep}->${stepId}`, source: dep, target: stepId } })
    }
  }

  const cy = window.cytoscape({
    container,
    elements: [...nodes, ...edges],
    style: [
      {
        selector: 'node',
        style: {
          label:           'data(label)',
          'text-valign':   'center',
          'text-halign':   'center',
          'background-color': 'data(bg)',
          'color':         'data(text)',
          'font-size':     '11px',
          'font-weight':   'bold',
          width:           'label',
          height:          'label',
          padding:         '10px',
          shape:           'roundrectangle',
        }
      },
      {
        selector: 'node:selected',
        style: {
          'border-width': 3,
          'border-color': '#0f172a',
        }
      },
      {
        selector: 'edge',
        style: {
          width:                  2,
          'line-color':           '#cbd5e1',
          'target-arrow-color':   '#cbd5e1',
          'target-arrow-shape':   'triangle',
          'curve-style':          'bezier',
        }
      }
    ],
    layout: {
      name:          'dagre',
      rankDir:       'TB',
      padding:       20,
      spacingFactor: 1.25,
      nodeSep:       40,
      rankSep:       50,
    },
    userZoomingEnabled:   true,
    userPanningEnabled:   true,
    boxSelectionEnabled:  false,
  })

  if (onNodeClick) {
    cy.on('tap', 'node', (e) => onNodeClick(e.target.id()))
  }

  return cy
}

/**
 * Update node colors in an existing Cytoscape instance without re-rendering.
 * Call this during workflow auto-refresh.
 */
export function updateDagColors(cy, stepStates) {
  if (!cy) return
  cy.nodes().forEach(node => {
    const state  = stepStates[node.id()]
    const status = state?.status ?? 'pending'
    const colors = STATUS_COLORS[status] ?? STATUS_COLORS.pending
    node.data('bg',   colors.bg)
    node.data('text', colors.text)
    node.data('status', status)
  })
}
