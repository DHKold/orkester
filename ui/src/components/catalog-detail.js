import { esc } from '../utils.js'
import { openModal } from './modal.js'
import { renderDag } from './dag.js'

// ── Work Detail Modal ────────────────────────────────────────────────────────

export function showWorkDetail(work) {
  const steps   = work.spec?.steps  ?? []
  const inputs  = Array.isArray(work.spec?.inputs)  ? work.spec.inputs  : []
  const outputs = Array.isArray(work.spec?.outputs) ? work.spec.outputs : []
  const tags    = work.metadata?.tags ?? []
  const owner   = work.metadata?.owner ?? ''
  const html = `
    <div class="row" style="margin-bottom:0.75rem;flex-wrap:wrap;gap:0.4rem;align-items:center">
      <span class="tag">${esc(work.version ?? '')}</span>
      <span class="muted" style="font-size:0.85rem">${steps.length} step${steps.length !== 1 ? 's' : ''}</span>
      ${tags.map(t => `<span class="tag" style="background:#dbeafe;color:#1e40af">${esc(t)}</span>`).join('')}
      ${owner ? `<span class="muted" style="font-size:0.82rem">owner: <strong>${esc(owner)}</strong></span>` : ''}
    </div>
    ${work.metadata?.description ? `<p style="margin-bottom:0.75rem">${esc(work.metadata.description)}</p>` : ''}
    ${workInputsHtml(inputs)}${workOutputsHtml(outputs)}${workStepsHtml(steps)}
    <details open style="margin-bottom:0.5rem">
      <summary><strong>Execution Graph</strong></summary>
      <div id="work-detail-dag" style="width:100%;height:280px;border:1px solid var(--pico-muted-border-color,#e2e8f0);border-radius:0.5rem;background:#f8fafc;margin-top:0.5rem"></div>
    </details>`
  openModal(work.name, html)
  const dagEl = document.getElementById('work-detail-dag')
  if (dagEl) renderDag(dagEl, work, {}, null)
}

function workInputsHtml(inputs) {
  if (!inputs.length) return ''
  const rows = inputs.map(i => `<tr>
    <td><code>${esc(i.name)}</code>${i.required === false ? ' <span class="tag" style="font-size:0.75rem">optional</span>' : ''}</td>
    <td class="muted">${esc(i.description ?? '—')}</td>
    <td><code>${esc(i.type ?? i.input_type ?? '—')}</code></td>
    <td>${i.default != null ? `<code>${esc(JSON.stringify(i.default))}</code>` : '<span class="muted">—</span>'}</td>
  </tr>`).join('')
  return `<details open style="margin-bottom:0.75rem"><summary><strong>Inputs</strong> <span class="muted">(${inputs.length})</span></summary>
    <figure style="margin-top:0.5rem"><table><thead><tr><th>Name</th><th>Description</th><th>Type</th><th>Default</th></tr></thead><tbody>${rows}</tbody></table></figure>
  </details>`
}

function workOutputsHtml(outputs) {
  if (!outputs.length) return ''
  const rows = outputs.map(o => `<tr>
    <td><code>${esc(o.name)}</code></td><td class="muted">${esc(o.description ?? '—')}</td>
    <td><code>${esc(o.type ?? o.output_type ?? '—')}</code></td>
  </tr>`).join('')
  return `<details style="margin-bottom:0.75rem"><summary><strong>Outputs</strong> <span class="muted">(${outputs.length})</span></summary>
    <figure style="margin-top:0.5rem"><table><thead><tr><th>Name</th><th>Description</th><th>Type</th></tr></thead><tbody>${rows}</tbody></table></figure>
  </details>`
}

function workStepsHtml(steps) {
  if (!steps.length) return ''
  const cards = steps.map(stepCardHtml).join('')
  return `<details open style="margin-bottom:0.75rem"><summary><strong>Steps</strong> <span class="muted">(${steps.length})</span></summary>
    <div style="margin-top:0.5rem">${cards}</div></details>`
}

function stepCardHtml(s) {
  const deps   = s.depends_on ?? s.dependsOn ?? []
  const inMap  = s.input_mapping  ?? s.inputMapping  ?? []
  const outMap = s.output_mapping ?? s.outputMapping ?? []
  return `<div style="border-left:3px solid var(--pico-primary,#3b82f6);padding:0.4rem 0.75rem;margin-bottom:0.5rem;background:var(--pico-card-background,#fff);border-radius:0 4px 4px 0">
    <div class="row" style="gap:0.5rem;flex-wrap:wrap;align-items:center">
      <strong>${esc(s.name)}</strong>
      <code class="muted" style="font-size:0.8rem">${esc(s.task_ref ?? s.taskRef ?? '—')}</code>
      ${deps.length ? `<span class="muted" style="font-size:0.78rem">← ${deps.map(d => `<code>${esc(d)}</code>`).join(', ')}</span>` : ''}
    </div>
    ${inMap.length  ? `<div style="font-size:0.78rem;margin-top:0.25rem"><span class="muted">inputs:</span> ${inMap.map(m => `<code>${esc(m.name)}</code>`).join(', ')}</div>` : ''}
    ${outMap.length ? `<div style="font-size:0.78rem"><span class="muted">outputs:</span> ${outMap.map(m => `<code>${esc(m.name)}</code>`).join(', ')}</div>` : ''}
  </div>`
}

// ── Task Detail Modal ────────────────────────────────────────────────────────

export function showTaskDetail(task) {
  const inputs  = task.spec?.inputs  ?? []
  const outputs = task.spec?.outputs ?? []
  const tags    = task.metadata?.tags ?? []
  const kind    = task.spec?.execution?.kind ?? '—'
  const html = `
    <div class="row" style="margin-bottom:0.75rem;flex-wrap:wrap;gap:0.4rem;align-items:center">
      <span class="tag">${esc(task.version ?? '')}</span>
      <code class="muted">${esc(kind)}</code>
      ${tags.map(t => `<span class="tag" style="background:#dbeafe;color:#1e40af">${esc(t)}</span>`).join('')}
    </div>
    ${task.metadata?.description ? `<p style="margin-bottom:0.75rem">${esc(task.metadata.description)}</p>` : ''}
    ${taskInputsHtml(inputs)}${taskOutputsHtml(outputs)}`
  openModal(task.name, html)
}

function taskInputsHtml(inputs) {
  if (!inputs.length) return '<p class="muted">No declared inputs.</p>'
  const rows = inputs.map(i => `<tr>
    <td><code>${esc(i.name)}</code>${!i.required ? ' <span class="tag" style="font-size:0.75rem">optional</span>' : ''}</td>
    <td class="muted">${esc(i.description ?? '—')}</td>
    <td><code>${esc(i.param_type ?? i.type ?? '—')}</code></td>
    <td>${i.default != null ? `<code>${esc(JSON.stringify(i.default))}</code>` : '<span class="muted">—</span>'}</td>
  </tr>`).join('')
  return `<details open style="margin-bottom:0.75rem"><summary><strong>Inputs</strong> <span class="muted">(${inputs.length})</span></summary>
    <figure style="margin-top:0.5rem"><table><thead><tr><th>Name</th><th>Description</th><th>Type</th><th>Default</th></tr></thead><tbody>${rows}</tbody></table></figure>
  </details>`
}

function taskOutputsHtml(outputs) {
  if (!outputs.length) return ''
  const rows = outputs.map(o => `<tr>
    <td><code>${esc(o.name)}</code></td><td class="muted">${esc(o.description ?? '—')}</td>
    <td><code>${esc(o.output_type ?? o.type ?? '—')}</code></td>
  </tr>`).join('')
  return `<details open style="margin-bottom:0.75rem"><summary><strong>Outputs</strong> <span class="muted">(${outputs.length})</span></summary>
    <figure style="margin-top:0.5rem"><table><thead><tr><th>Name</th><th>Description</th><th>Type</th></tr></thead><tbody>${rows}</tbody></table></figure>
  </details>`
}
