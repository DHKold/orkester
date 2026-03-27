use std::{
    sync::{atomic::{AtomicUsize, Ordering}, Arc},
    time::Duration,
};

use chrono::Utc;
use crossbeam_channel::Receiver;
use workaholic::{
    domain::work::FailureMode,
    domain::task::ExecutionKind,
    execution::{
        task_run::{TaskRun, TaskRunPhase, TaskRunSpec, TaskRunStatus, TaskRunError},
        work_run::{WorkRunPhase, WorkRunStatus},
        WorkRun,
    },
    traits,
};

use crate::{
    task_runner::runner_components::{RunnerExecuteRequest, RunnerExecuteResponse},
    workflow_server::dag::topological_sort,
};

use super::{WorkRunnerConfig, WorkRunnerContext, WorkRunnerHandle};

// ── Persistence collection names ──────────────────────────────────────────────

pub const WORK_RUNS: &str = "work_runs";
pub const TASK_RUNS: &str = "task_runs";

// ── ThreadWorkRunner spawner ──────────────────────────────────────────────────────

pub struct ThreadWorkRunner;

impl ThreadWorkRunner {
    /// Spawn a background thread workRunner and return a `WorkRunnerHandle` for it.
    pub fn spawn(cfg: &WorkRunnerConfig, ctx: WorkRunnerContext) -> WorkRunnerHandle {
        let (tx, rx) = crossbeam_channel::bounded::<String>(cfg.max_work_runs * 2);
        let active_count = Arc::new(AtomicUsize::new(0));
        let active_clone = active_count.clone();
        let name_clone = cfg.name.clone();

        let thread = std::thread::Builder::new()
            .name(format!("workRunner-{}", cfg.name))
            .spawn(move || {
                workRunner_loop(rx, ctx, active_clone);
            })
            .unwrap_or_else(|e| panic!("failed to spawn workRunner thread '{}': {e}", name_clone));

        WorkRunnerHandle {
            name: cfg.name.clone(),
            kind: cfg.kind.clone(),
            sender: tx,
            active_count,
            max_work_runs: cfg.max_work_runs,
            thread: Some(thread),
        }
    }
}

// ── WorkRunner loop ───────────────────────────────────────────────────────────────

fn workRunner_loop(
    rx: Receiver<String>,
    ctx: WorkRunnerContext,
    active_count: Arc<AtomicUsize>,
) {
    ctx.host.log("info", &format!("workRunner/{}", ctx.workRunner_name), "started");
    loop {
        let work_run_id = match rx.recv_timeout(Duration::from_millis(500)) {
            Ok(id) => id,
            Err(crossbeam_channel::RecvTimeoutError::Timeout) => continue,
            Err(crossbeam_channel::RecvTimeoutError::Disconnected) => break,
        };

        active_count.fetch_add(1, Ordering::Relaxed);
        ctx.host.log("info", &format!("workRunner/{}", ctx.workRunner_name),
            &format!("starting work run '{work_run_id}'"));

        execute_work_run(&work_run_id, &ctx);

        active_count.fetch_sub(1, Ordering::Relaxed);
    }
    ctx.host.log("info", &format!("workRunner/{}", ctx.workRunner_name), "stopped");
}

// ── Work run execution ────────────────────────────────────────────────────────

fn execute_work_run(work_run_id: &str, ctx: &WorkRunnerContext) {
    let mut work_run: WorkRun = match traits::retrieve(&*ctx.persistence, WORK_RUNS, work_run_id) {
        Ok(Some(r)) => r,
        Ok(None) => {
            ctx.host.log("error", &format!("workRunner/{}", ctx.workRunner_name),
                &format!("work run '{work_run_id}' not found"));
            return;
        }
        Err(e) => {
            ctx.host.log("error", &format!("workRunner/{}", ctx.workRunner_name),
                &format!("failed to load work run '{work_run_id}': {e}"));
            return;
        }
    };

    // Transition to Running.
    {
        let status = work_run.status.get_or_insert_with(WorkRunStatus::default);
        status.phase = WorkRunPhase::Running;
        status.started_at = Some(Utc::now());
        status.workRunner = Some(ctx.workRunner_name.clone());
    }
    if let Err(e) = traits::persist(&*ctx.persistence, WORK_RUNS, work_run_id, &work_run) {
        ctx.host.log("error", &format!("workRunner/{}", ctx.workRunner_name),
            &format!("failed to persist work run state: {e}"));
    }

    let (namespace, work_name) = parse_ref(&work_run.spec.work_ref, &work_run.namespace);

    let work = match ctx.host.call::<_, workaholic::domain::Work>(
        "catalog/GetWork",
        serde_json::json!({ "namespace": namespace, "name": work_name }),
    ) {
        Ok(w) => w,
        Err(e) => {
            fail_work_run(ctx, &mut work_run, &format!("catalog lookup failed: {e}"));
            return;
        }
    };

    let failure_mode = work.spec.failure_policy.mode.clone();
    let tasks = work.spec.tasks;

    if tasks.is_empty() {
        succeed_work_run(ctx, &mut work_run);
        return;
    }

    let order = match topological_sort(&tasks) {
        Ok(o) => o,
        Err(e) => {
            fail_work_run(ctx, &mut work_run, &format!("DAG error: {e}"));
            return;
        }
    };

    let mut any_failed = false;

    for &task_idx in &order {
        let work_task = &tasks[task_idx];

        if any_failed && failure_mode == FailureMode::FailFast {
            let tr_id = task_run_id(work_run_id, &work_task.name, 1);
            let mut task_run = make_task_run(&tr_id, &work_run, work_task, 1);
            task_run.status.get_or_insert_with(TaskRunStatus::default).phase = TaskRunPhase::Skipped;
            save_task_run(ctx, &task_run);
            continue;
        }

        let retry_count = work_task.retry_count.unwrap_or(0);
        let ok = run_task_with_retry(ctx, work_run_id, &work_run, work_task, retry_count);
        if !ok {
            any_failed = true;
        }
    }

    if any_failed {
        fail_work_run(ctx, &mut work_run, "one or more tasks failed");
    } else {
        succeed_work_run(ctx, &mut work_run);
    }
}

// ── Task execution with retry ─────────────────────────────────────────────────

fn run_task_with_retry(
    ctx: &WorkRunnerContext,
    work_run_id: &str,
    work_run: &WorkRun,
    work_task: &workaholic::domain::work::WorkTask,
    retry_count: u32,
) -> bool {
    let (task_ns, task_name) = parse_ref(&work_task.task_ref, &work_run.namespace);

    for attempt in 1..=(retry_count + 1) {
        let tr_id = task_run_id(work_run_id, &work_task.name, attempt);
        let mut task_run = make_task_run(&tr_id, work_run, work_task, attempt);

        // Fetch Task definition via catalog.
        let task_def = match ctx.host.call::<_, workaholic::domain::Task>(
            "catalog/GetTask",
            serde_json::json!({ "namespace": task_ns, "name": task_name }),
        ) {
            Ok(t) => t,
            Err(e) => {
                ctx.host.log("error", &format!("workRunner/{}", ctx.workRunner_name),
                    &format!("catalog/GetTask failed for '{}': {e}", work_task.task_ref));
                mark_failed(ctx, &mut task_run, "CATALOG_ERROR", &e.to_string());
                save_task_run(ctx, &task_run);
                if attempt > retry_count { return false; }
                continue;
            }
        };

        // Merge execution config + work-task inputs so the runner gets everything.
        let merged = merge_inputs(&work_task.inputs, &task_def.spec.execution.config);
        task_run.spec.inputs = merged.clone();

        // Transition to Running.
        {
            let s = task_run.status.get_or_insert_with(TaskRunStatus::default);
            s.phase = TaskRunPhase::Running;
            s.started_at = Some(Utc::now());
            s.workRunner = Some(ctx.workRunner_name.clone());
        }
        save_task_run(ctx, &task_run);

        ctx.host.log("info", &format!("workRunner/{}", ctx.workRunner_name),
            &format!("task '{}' attempt {attempt}/{}", work_task.name, retry_count + 1));

        // Map the task's execution kind to the corresponding component kind string,
        // then ask the host to create a fresh runner instance for this task.
        // The runner is dropped when this block exits (OwnedComponent::drop calls free).
        let runner_kind = runner_component_kind(&task_def.spec.execution.kind);
        let runner = match ctx.host.create_component(runner_kind, serde_json::json!({})) {
            Some(r) => r,
            None => {
                ctx.host.log("error", &format!("workRunner/{}", ctx.workRunner_name),
                    &format!("failed to create runner '{runner_kind}' for task '{}' attempt {attempt}",
                        work_task.name));
                mark_failed(ctx, &mut task_run, "RUNNER_CREATE_ERROR",
                    &format!("no factory for runner kind '{runner_kind}'"));
                save_task_run(ctx, &task_run);
                if attempt > retry_count { return false; }
                continue;
            }
        };

        // Dispatch execution synchronously — the runner executes the task and
        // returns once it finishes, times out, or fails.
        let run_result: RunnerExecuteResponse = match runner.call("Execute", RunnerExecuteRequest {
            task_name: work_task.name.clone(),
            inputs:    merged.clone(),
        }) {
            Ok(r) => r,
            Err(e) => {
                ctx.host.log("error", &format!("workRunner/{}", ctx.workRunner_name),
                    &format!("runner Execute failed for task '{}' attempt {attempt}: {e}",
                        work_task.name));
                mark_failed(ctx, &mut task_run, "RUNNER_ERROR", &e.to_string());
                save_task_run(ctx, &task_run);
                if attempt > retry_count { return false; }
                continue;
            }
        };
        // runner is dropped here, freeing the on-demand component.

        let succeeded = run_result.phase == TaskRunPhase::Succeeded;

        // Persist the final task run state.
        {
            let s = task_run.status.get_or_insert_with(TaskRunStatus::default);
            s.phase       = run_result.phase.clone();
            s.outputs     = run_result.outputs;
            s.external_id = run_result.external_id;
            s.error       = run_result.error;
            s.finished_at = Some(Utc::now());
            s.task_runner = Some(format!("{:?}", task_def.spec.execution.kind));
        }
        save_task_run(ctx, &task_run);

        if succeeded {
            ctx.host.log("info", &format!("workRunner/{}", ctx.workRunner_name),
                &format!("task '{}' succeeded", work_task.name));
            return true;
        }

        let err_msg = task_run
            .status.as_ref()
            .and_then(|s| s.error.as_ref())
            .map(|e| e.message.clone())
            .unwrap_or_default();

        ctx.host.log("warn", &format!("workRunner/{}", ctx.workRunner_name),
            &format!("task '{}' attempt {attempt} failed: {err_msg}", work_task.name));
    }
    false
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn task_run_id(work_run_id: &str, task_name: &str, attempt: u32) -> String {
    format!("{work_run_id}-{task_name}-{attempt}")
}

fn make_task_run(
    tr_id: &str,
    work_run: &WorkRun,
    work_task: &workaholic::domain::work::WorkTask,
    attempt: u32,
) -> TaskRun {
    TaskRun {
        kind: "orkester/taskrun:1.0".into(),
        name: tr_id.into(),
        namespace: work_run.namespace.clone(),
        version: "1.0.0".into(),
        metadata: Default::default(),
        spec: TaskRunSpec {
            work_run_ref: work_run.name.clone(),
            task_name: work_task.name.clone(),
            task_ref: work_task.task_ref.clone(),
            attempt,
            inputs: work_task.inputs.clone(),
        },
        status: Some(TaskRunStatus { phase: TaskRunPhase::Pending, ..Default::default() }),
    }
}

fn mark_failed(ctx: &WorkRunnerContext, run: &mut TaskRun, code: &str, msg: &str) {
    let s = run.status.get_or_insert_with(TaskRunStatus::default);
    s.phase = TaskRunPhase::Failed;
    s.finished_at = Some(Utc::now());
    s.error = Some(TaskRunError { code: code.into(), message: msg.into() });
    ctx.host.log("error", &format!("workRunner/{}", ctx.workRunner_name),
        &format!("task '{}' failed: {msg}", run.spec.task_name));
}

fn save_task_run(ctx: &WorkRunnerContext, run: &TaskRun) {
    if let Err(e) = traits::persist(&*ctx.persistence, TASK_RUNS, &run.name, run) {
        ctx.host.log("warn", &format!("workRunner/{}", ctx.workRunner_name),
            &format!("failed to persist task run '{}': {e}", run.name));
    }
}

fn fail_work_run(ctx: &WorkRunnerContext, run: &mut WorkRun, reason: &str) {
    let s = run.status.get_or_insert_with(WorkRunStatus::default);
    s.phase = WorkRunPhase::Failed;
    s.finished_at = Some(Utc::now());
    s.error = Some(reason.to_string());
    if let Err(e) = traits::persist(&*ctx.persistence, WORK_RUNS, &run.name, run) {
        ctx.host.log("error", &format!("workRunner/{}", ctx.workRunner_name),
            &format!("failed to persist failed work run '{}': {e}", run.name));
    }
    ctx.host.log("error", &format!("workRunner/{}", ctx.workRunner_name),
        &format!("work run '{}' failed: {reason}", run.name));
}

fn succeed_work_run(ctx: &WorkRunnerContext, run: &mut WorkRun) {
    let s = run.status.get_or_insert_with(WorkRunStatus::default);
    s.phase = WorkRunPhase::Succeeded;
    s.finished_at = Some(Utc::now());
    s.task_counts.running = 0;
    if let Err(e) = traits::persist(&*ctx.persistence, WORK_RUNS, &run.name, run) {
        ctx.host.log("error", &format!("workRunner/{}", ctx.workRunner_name),
            &format!("failed to persist succeeded work run '{}': {e}", run.name));
    }
    ctx.host.log("info", &format!("workRunner/{}", ctx.workRunner_name),
        &format!("work run '{}' succeeded", run.name));
}

fn parse_ref<'a>(r: &'a str, default_ns: &'a str) -> (&'a str, &'a str) {
    if let Some(pos) = r.find('/') {
        (&r[..pos], &r[pos + 1..])
    } else {
        (default_ns, r)
    }
}

fn merge_inputs(work_task: &serde_json::Value, exec_config: &serde_json::Value) -> serde_json::Value {
    let mut merged = exec_config.clone();
    if let (Some(m), Some(inputs)) = (merged.as_object_mut(), work_task.as_object()) {
        for (k, v) in inputs {
            m.insert(k.clone(), v.clone());
        }
    }
    merged
}

// ── Runner kind mapping ───────────────────────────────────────────────────────

/// Map an [`ExecutionKind`] to the component kind string used by
/// `orkester/CreateComponent` to create an on-demand runner.
fn runner_component_kind(kind: &ExecutionKind) -> &'static str {
    match kind {
        ExecutionKind::Shell      => "workaholic/ShellRunner:1.0",
        ExecutionKind::Container  => "workaholic/ContainerRunner:1.0",
        ExecutionKind::Kubernetes => "workaholic/KubernetesRunner:1.0",
        _ => "workaholic/ShellRunner:1.0",
    }
}
