// ── DAG visualization ──────────────────────────────────────────────────────
//
// Renders a WorkRun execution graph using Cytoscape.js + cytoscape-dagre.
//
// Usage:
//   renderDag('dag-container', steps, workSpec)
//   // Later, to update node colours without full re-render:
//   updateDagColors('dag-container', steps)

/** Map step states to node background colours (matches --status-* CSS vars). */
const STATE_COLORS = {
  pending:   '#94a3b8',
  running:   '#3b82f6',
  succeeded: '#22c55e',
  failed:    '#ef4444',
  cancelled: '#6b7280',
  skipped:   '#f59e0b',
}

const instances = {}  // containerId → cytoscape instance

/**
 * Render (or re-render) the DAG inside a container element.
 *
 * @param {string} containerId  - ID of a .dag-container element
 * @param {Array}  steps        - WorkRun.status.steps[]
 * @param {Object} workSpec     - Work.spec (for step dependencies)
 * @param {Function} [onStepClick] - callback(stepName: string)
 */
export function renderDag(containerId, steps = [], workSpec = {}, onStepClick) {
  const container = document.getElementById(containerId)
  if (!container) return

  // Destroy existing instance
  if (instances[containerId]) {
    instances[containerId].destroy()
    delete instances[containerId]
  }

  const specSteps = workSpec.steps ?? []

  // Build Cytoscape elements
  const elements = []

  // Nodes
  for (const specStep of specSteps) {
    const runStep = steps.find(s => s.name === specStep.name)
    const state   = runStep?.state ?? 'pending'
    elements.push({
      data: {
        id:    specStep.name,
        label: specStep.name,
        state,
        color: STATE_COLORS[state] ?? STATE_COLORS.pending,
      },
    })
  }

  // Edges (dependsOn)
  for (const specStep of specSteps) {
    for (const dep of specStep.dependsOn ?? []) {
      elements.push({
        data: {
          id:     `${dep}->${specStep.name}`,
          source: dep,
          target: specStep.name,
        },
      })
    }
  }

  const cy = cytoscape({
    container,
    elements,
    layout: { name: 'dagre', rankDir: 'LR', nodeSep: 40, rankSep: 60, padding: 20 },
    style: [
      {
        selector: 'node',
        style: {
          'label':            'data(label)',
          'background-color': 'data(color)',
          'color':            '#fff',
          'text-valign':      'center',
          'text-halign':      'center',
          'font-size':        '11px',
          'font-weight':      'bold',
          'width':            'label',
          'height':           28,
          'padding':          '6px 10px',
          'shape':            'round-rectangle',
          'text-outline-width': 0,
        },
      },
      {
        selector: 'edge',
        style: {
          'width':             2,
          'line-color':        '#cbd5e1',
          'target-arrow-color':'#cbd5e1',
          'target-arrow-shape':'triangle',
          'curve-style':       'bezier',
        },
      },
      {
        selector: 'node:selected',
        style: {
          'border-width': 3,
          'border-color': '#fff',
        },
      },
    ],
    userZoomingEnabled:   true,
    userPanningEnabled:   true,
    boxSelectionEnabled:  false,
  })

  if (onStepClick) {
    cy.on('tap', 'node', (e) => onStepClick(e.target.data('id')))
  }

  // Add reset button handler
  const resetBtn = container.parentElement?.querySelector('[data-dag-reset]')
  if (resetBtn) {
    resetBtn.onclick = () => cy.fit(undefined, 20)
  }

  instances[containerId] = cy
}

/**
 * Update node colours in an existing DAG without full re-render.
 * Call this during auto-refresh polling.
 *
 * @param {string} containerId
 * @param {Array}  steps - WorkRun.status.steps[]
 */
export function updateDagColors(containerId, steps = []) {
  const cy = instances[containerId]
  if (!cy) return
  for (const step of steps) {
    const node = cy.getElementById(step.name)
    if (node.length) {
      const color = STATE_COLORS[step.state] ?? STATE_COLORS.pending
      node.data('color', color).data('state', step.state)
      node.style('background-color', color)
    }
  }
}

/** Destroy a DAG instance (call in setCleanup). */
export function destroyDag(containerId) {
  if (instances[containerId]) {
    instances[containerId].destroy()
    delete instances[containerId]
  }
}
