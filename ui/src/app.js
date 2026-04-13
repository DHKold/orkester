// ── Application entry point ────────────────────────────────────────────────

import { route, start }           from './router.js'
import { initSidebar }            from './components/sidebar.js'

import { renderDashboard }        from './pages/dashboard.js'
import { renderCatalog }          from './pages/catalog/index.js'
import { renderCatalogDetail }    from './pages/catalog/detail.js'
import { renderRunners }          from './pages/workspace/runners.js'
import { renderRunnerDetail }     from './pages/workspace/runner-detail.js'
import { renderTaskRunnerDetail } from './pages/workspace/task-runner-detail.js'
import { renderWorkRuns }         from './pages/workspace/work-runs.js'
import { renderWorkRunDetail }    from './pages/workspace/work-run-detail.js'
import { renderCrons }            from './pages/workspace/crons.js'
import { renderCronDetail }       from './pages/workspace/cron-detail.js'
import { renderMetrics }          from './pages/metrics.js'
import { renderSettings }         from './pages/settings.js'
import { renderHelp }             from './pages/help.js'

// ── Routes ─────────────────────────────────────────────────────────────────
route('/',                                      ()             => renderDashboard())
route('/catalog',                               ({ query })    => renderCatalog({ query }))
route('/catalog/detail',                        ({ query })    => renderCatalogDetail({ query }))
route('/workspace/runners',                     ()             => renderRunners())
route('/workspace/runners/:name',               ({ name })     => renderRunnerDetail({ name }))
route('/workspace/task-runners/:name',          ({ name })     => renderTaskRunnerDetail({ name }))
route('/workspace/work-runs',                   ()             => renderWorkRuns())
route('/workspace/work-runs/:name',             ({ name })     => renderWorkRunDetail({ name }))
route('/workspace/crons',                       ()             => renderCrons())
route('/workspace/crons/:name',                 ({ name })     => renderCronDetail({ name }))
route('/metrics',                               ()             => renderMetrics())
route('/settings',                              ({ query })    => renderSettings({ query }))
route('/help',                                  ({ query })    => renderHelp({ query }))

// ── Boot ───────────────────────────────────────────────────────────────────
initSidebar()
start()
