use std::{
    sync::{atomic::{AtomicUsize, Ordering}, Arc},
    time::Duration,
};

use chrono::Utc;
use crossbeam_channel::Receiver;
use workaholic::{
    domain::work::FailureMode,
    execution::{
        task_run::{TaskRun, TaskRunPhase, TaskRunSpec, TaskRunStatus, TaskRunError},
        work_run::{WorkRunPhase, WorkRunStatus},
        WorkRun,
    },
    traits,
};

use crate::{
    task_runner::{TaskRunEvent, build_runner},
    workflow_server::dag::topological_sort,
};

use super::{WorkerConfig, WorkerContext, WorkerHandle};

// ── Persistence collection names ──────────────────────────────────────────────

pub const WORK_RUNS: &str = "work_runs";
pub const TASK_RUNS: &str = "task_runs";

// ── ThreadWorker spawner ──────────────────────────────────────────────────────

pub struct ThreadWorker;

impl ThreadWorker {
    /// Spawn a background thread worker and return a `WorkerHandle` for it.
    pub fn spawn(cfg: &WorkerConfig, ctx: WorkerContext) -> WorkerHandle {
        let (tx, rx) = crossbeam_channel::bounded::<String>(cfg.max_work_runs * 2);
        let active_count = Arc::new(AtomicUsize::new(0));
        let active_clone = active_count.clone();
        let name_clone = cfg.name.clone();

        let thread = std::thread::Builder::new()
            .name(format!("worker-{}", cfg.name))
            .spawn(move || {
                worker_loop(rx, ctx, active_clone);
            })
            .unwrap_or_else(|e| panic!("failed to spawn worker thread '{}': {e}", name_clone));

        WorkerHandle {
            name: cfg.name.clone(),
            kind: cfg.kind.clone(),
            sender: tx,
            active_count,
            max_work_runs: cfg.max_work_runs,
            thread: Some(thread),
        }
    }
}

// ── Worker loop ───────────────────────────────────────────────────────────────

fn worker_loop(
    rx: Receiver<String>,
    ctx: WorkerContext,
    active_count: Arc<AtomicUsize>,
) {
    log::info!("[worker/{}] started", ctx.worker_name);
    loop {
        let work_run_id = match rx.recv_timeout(Duration::from_millis(500)) {
            Ok(id) => id,
            Err(crossbeam_channel::RecvTimeoutError::Timeout) => continue,
            Err(crossbeam_channel::RecvTimeoutError::Disconnected) => break,
        };

        active_count.fetch_add(1, Ordering::Relaxed);
        ctx.host.log("info", &format!("worker/{}", ctx.worker_name),
            &format!("starting work run '{work_run_id}'"));

        execute_work_run(&work_run_id, &ctx);

        active_count.fetch_sub(1, Ordering::Relaxed);
    }
    log::info!("[worker/{}] stopped", ctx.worker_name);
}

// ── Work run execution ────────────────────────────────────────────────────────

fn execute_work_run(work_run_id: &str, ctx: &WorkerContext) {
    let mut work_run: WorkRun = match traits::retrieve(&*ctx.persistence, WORK_RUNS, work_run_id) {
        Ok(Some(r)) => r,
        Ok(None) => {
            log::error!("[worker/{}] work run '{work_run_id}' not found", ctx.worker_name);
            return;
        }
        Err(e) => {
            log::error!("[worker/{}] failed to load work run '{work_run_id}': {e}", ctx.worker_name);
            return;
        }
    };

    // Transition to Running.
    {
        let status = work_run.status.get_or_insert_with(WorkRunStatus::default);
        status.phase = WorkRunPhase::Running;
        status.started_at = Some(Utc::now());
        status.worker = Some(ctx.worker_name.clone());
    }
    if let Err(e) = traits::persist(&*ctx.persistence, WORK_RUNS, work_run_id, &work_run) {
        log::error!("[worker/{}] failed to persist work run state: {e}", ctx.worker_name);
    }

    let (namespace, work_name) = parse_ref(&work_run.spec.work_ref, "default");

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
    ctx: &WorkerContext,
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
                log::error!("[worker/{}] catalog/GetTask failed for '{}': {e}", ctx.worker_name, work_task.task_ref);
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
            s.worker = Some(ctx.worker_name.clone());
        }
        save_task_run(ctx, &task_run);

        ctx.host.log("info", &format!("worker/{}", ctx.worker_name),
            &format!("task '{}' attempt {attempt}/{}", work_task.name, retry_count + 1));

        // Spawn the task runner (non-blocking).
        let mut runner = build_runner(&task_def.spec.execution.kind);
        let handle = runner.spawn(&work_task.name, &merged);

        // Subscribe and forward events as log messages while waiting.
        let events = handle.subscribe();
        std::thread::spawn({
            let host = ctx.host.clone();
            let source = format!("worker/{}", ctx.worker_name);
            move || {
                for event in events {
                    match event {
                        TaskRunEvent::LogLine { level, message } => {
                            host.log(&level, &source, &message);
                        }
                        TaskRunEvent::ExternalId(id) => {
                            log::debug!("[{source}] external_id={id}");
                        }
                        TaskRunEvent::PhaseChanged(p) => {
                            log::debug!("[{source}] phase -> {p:?}");
                        }
                    }
                }
            }
        });

        // Block until done.
        let result = handle.wait();
        let succeeded = result.phase == TaskRunPhase::Succeeded;

        // Apply result to task run.
        {
            let s = task_run.status.get_or_insert_with(TaskRunStatus::default);
            s.phase = result.phase.clone();
            s.outputs = result.outputs;
            s.external_id = result.external_id;
            s.error = result.error;
            s.finished_at = Some(Utc::now());
            s.task_runner = Some(runner.kind().to_string());
        }
        save_task_run(ctx, &task_run);

        if succeeded {
            ctx.host.log("info", &format!("worker/{}", ctx.worker_name),
                &format!("task '{}' succeeded", work_task.name));
            return true;
        }

        let err_msg = task_run
            .status.as_ref()
            .and_then(|s| s.error.as_ref())
            .map(|e| e.message.clone())
            .unwrap_or_default();

        ctx.host.log("warn", &format!("worker/{}", ctx.worker_name),
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

fn mark_failed(ctx: &WorkerContext, run: &mut TaskRun, code: &str, msg: &str) {
    let s = run.status.get_or_insert_with(TaskRunStatus::default);
    s.phase = TaskRunPhase::Failed;
    s.finished_at = Some(Utc::now());
    s.error = Some(TaskRunError { code: code.into(), message: msg.into() });
    ctx.host.log("error", &format!("worker/{}", ctx.worker_name),
        &format!("task '{}' failed: {msg}", run.spec.task_name));
}

fn save_task_run(ctx: &WorkerContext, run: &TaskRun) {
    if let Err(e) = traits::persist(&*ctx.persistence, TASK_RUNS, &run.name, run) {
        log::warn!("[worker/{}] failed to persist task run '{}': {e}", ctx.worker_name, run.name);
    }
}

fn fail_work_run(ctx: &WorkerContext, run: &mut WorkRun, reason: &str) {
    let s = run.status.get_or_insert_with(WorkRunStatus::default);
    s.phase = WorkRunPhase::Failed;
    s.finished_at = Some(Utc::now());
    s.error = Some(reason.to_string());
    if let Err(e) = traits::persist(&*ctx.persistence, WORK_RUNS, &run.name, run) {
        log::error!("[worker/{}] failed to persist failed work run: {e}", ctx.worker_name);
    }
    ctx.host.log("error", &format!("worker/{}", ctx.worker_name),
        &format!("work run '{}' failed: {reason}", run.name));
}

fn succeed_work_run(ctx: &WorkerContext, run: &mut WorkRun) {
    let s = run.status.get_or_insert_with(WorkRunStatus::default);
    s.phase = WorkRunPhase::Succeeded;
    s.finished_at = Some(Utc::now());
    s.task_counts.running = 0;
    if let Err(e) = traits::persist(&*ctx.persistence, WORK_RUNS, &run.name, run) {
        log::error!("[worker/{}] failed to persist succeeded work run: {e}", ctx.worker_name);
    }
    ctx.host.log("info", &format!("worker/{}", ctx.worker_name),
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
