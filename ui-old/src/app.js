import { route, start }        from './router.js'
import { initSidebar }         from './components/sidebar.js'
import { renderHealth }        from './pages/health.js'
import { renderMetrics }       from './pages/metrics.js'
import { renderNamespaces }    from './pages/namespaces.js'
import { renderNamespace }     from './pages/namespace.js'
import { renderWorkflows }     from './pages/workflows.js'
import { renderWorkflow }      from './pages/workflow.js'
import { renderCrons }         from './pages/crons.js'

// ── Routes ────────────────────────────────────────────────────────────────────
route('/',                             ()               => renderNamespaces())
route('/health',                       ()               => renderHealth())
route('/metrics',                      ()               => renderMetrics())
route('/namespaces',                   ()               => renderNamespaces())
route('/namespaces/:ns',               ({ ns })         => renderNamespace({ ns }))
route('/namespaces/:ns/workflows',     ({ ns, query })  => renderWorkflows({ ns, query }))
route('/namespaces/:ns/workflows/:id', ({ ns, id })     => renderWorkflow({ ns, id }))
route('/namespaces/:ns/crons',         ({ ns })         => renderCrons({ ns }))

initSidebar()
start()
