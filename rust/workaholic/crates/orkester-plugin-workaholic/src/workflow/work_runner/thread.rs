use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use uuid::Uuid;
use workaholic::{
    TaskRunDoc, WorkRunDoc, WorkRunRequestDoc, WorkRunState, WorkRunnerDoc, WorkRunnerSpec,
    WorkRunnerState, WorkRunnerStatus,
};

use super::traits::{
    WorkRun, WorkRunError, WorkRunEvent, WorkRunEventStream, WorkRunResources,
    WorkRunner, WorkRunnerError,
};

// ─── ThreadWorkRunner ──────────────────────────────────────────────────────────

/// A `WorkRunner` that executes workflow runs on OS threads.
///
/// The `ThreadWorkRunner` owns global resource accounting and the registry of
/// active `WorkRun`s.  It delegates per-workflow DAG logic to `ThreadWorkRun`.
#[derive(Debug)]
pub struct ThreadWorkRunner {
    name: String,
    namespace: String,
    spec: workaholic::WorkRunnerSpec,
    state: Arc<Mutex<ThreadWorkRunnerState>>,
}

#[derive(Debug, Default)]
struct ThreadWorkRunnerState {
    runner_state: WorkRunnerState,
    active_work_runs: HashMap<String, Arc<ThreadWorkRun>>,
}

impl ThreadWorkRunner {
    pub fn new(name: impl Into<String>, namespace: impl Into<String>, spec: WorkRunnerSpec) -> Self {
        Self {
            name: name.into(),
            namespace: namespace.into(),
            spec,
            state: Arc::new(Mutex::new(ThreadWorkRunnerState {
                runner_state: WorkRunnerState::Active,
                active_work_runs: HashMap::new(),
            })),
        }
    }

    fn self_ref(&self) -> String {
        format!("worker://{}/{}:1.0.0", self.namespace, self.name)
    }
}

impl WorkRunner for ThreadWorkRunner {
    fn as_doc(&self) -> WorkRunnerDoc {
        let state = self.state.lock().unwrap();
        WorkRunnerDoc {
            kind: workaholic::WORKER_KIND.to_string(),
            name: self.name.clone(),
            version: "1.0.0".to_string(),
            metadata: workaholic::DocumentMetadata {
                namespace: Some(self.namespace.clone()),
                owner: None,
                description: None,
                tags: vec![],
                extra: Default::default(),
            },
            spec: self.spec.clone(),
            status: Some(WorkRunnerStatus {
                state: state.runner_state.clone(),
                active_work_runs: state.active_work_runs.len(),
                active_task_runs: 0, // TODO: aggregate from active runs
                state_history: vec![],
            }),
        }
    }

    fn spawn(
        &self,
        request: WorkRunRequestDoc,
    ) -> Result<Box<dyn WorkRun>, WorkRunnerError> {
        let mut state = self.state.lock().unwrap();

        if state.runner_state != WorkRunnerState::Active {
            return Err(WorkRunnerError::NotActive);
        }

        let max = self.spec.concurrency.max_work_runs;
        if max > 0 && state.active_work_runs.len() >= max {
            return Err(WorkRunnerError::CapacityExceeded(
                format!("max_work_runs={} reached", max),
            ));
        }

        let run_name = Uuid::new_v4().to_string();
        let run = Arc::new(ThreadWorkRun::new(
            run_name.clone(),
            self.namespace.clone(),
            self.self_ref(),
            request,
        ));

        state.active_work_runs.insert(run_name, Arc::clone(&run));
        Ok(Box::new(ThreadWorkRunHandle(run)))
    }
}

// ─── ThreadWorkRun ─────────────────────────────────────────────────────────────

/// Internal state for one workflow execution.
#[derive(Debug)]
pub(crate) struct ThreadWorkRun {
    name: String,
    namespace: String,
    work_runner_ref: String,
    request: WorkRunRequestDoc,
    state: Mutex<ThreadWorkRunState>,
    /// Broadcast sender for events.  Receivers are handed out via `subscribe()`.
    sender: crossbeam_channel::Sender<WorkRunEvent>,
    receiver: crossbeam_channel::Receiver<WorkRunEvent>,
}

#[derive(Debug)]
struct ThreadWorkRunState {
    run_state: WorkRunState,
    steps: HashMap<String, WorkRunState>,
    attempts: HashMap<String, u32>,
    active_task_run_refs: HashMap<String, String>,
}

impl ThreadWorkRun {
    fn new(
        name: String,
        namespace: String,
        work_runner_ref: String,
        request: WorkRunRequestDoc,
    ) -> Self {
        let (tx, rx) = crossbeam_channel::unbounded();
        let steps: HashMap<String, WorkRunState> = request
            .spec
            .steps
            .iter()
            .map(|s| (s.name.clone(), WorkRunState::Pending))
            .collect();
        Self {
            name,
            namespace,
            work_runner_ref,
            request,
            state: Mutex::new(ThreadWorkRunState {
                run_state: WorkRunState::Pending,
                steps,
                attempts: HashMap::new(),
                active_task_run_refs: HashMap::new(),
            }),
            sender: tx,
            receiver: rx,
        }
    }
}

impl WorkRun for ThreadWorkRun {
    fn as_doc(&self) -> WorkRunDoc {
        let state = self.state.lock().unwrap();
        use workaholic::{WorkRunSpec, WorkRunStatus, WorkRunStepStatus, WorkRunSummary};

        let steps: Vec<WorkRunStepStatus> = self
            .request
            .spec
            .steps
            .iter()
            .map(|s| WorkRunStepStatus {
                name: s.name.clone(),
                state: state.steps.get(&s.name).cloned().unwrap_or(WorkRunState::Pending),
                task_run_request_ref: Some(s.task_run_request_ref.clone()),
                active_task_run_ref: state.active_task_run_refs.get(&s.name).cloned(),
                attempts: *state.attempts.get(&s.name).unwrap_or(&0),
            })
            .collect();

        let summary = WorkRunSummary {
            total_steps: steps.len(),
            pending_steps: steps.iter().filter(|s| s.state == WorkRunState::Pending).count(),
            running_steps: steps.iter().filter(|s| s.state == WorkRunState::Running).count(),
            succeeded_steps: steps.iter().filter(|s| s.state == WorkRunState::Succeeded).count(),
            failed_steps: steps.iter().filter(|s| s.state == WorkRunState::Failed).count(),
            cancelled_steps: steps.iter().filter(|s| s.state == WorkRunState::Cancelled).count(),
        };

        WorkRunDoc {
            kind: workaholic::WORK_RUN_KIND.to_string(),
            name: self.name.clone(),
            version: "1.0.0".to_string(),
            metadata: workaholic::DocumentMetadata {
                namespace: Some(self.namespace.clone()),
                owner: None,
                description: None,
                tags: vec![],
                extra: Default::default(),
            },
            spec: WorkRunSpec {
                work_run_request_ref: self.request.name.clone(),
                work_ref: self.request.spec.work_ref.clone(),
                work_runner_ref: self.work_runner_ref.clone(),
                trigger: self.request.spec.trigger.clone(),
            },
            status: Some(WorkRunStatus {
                state: state.run_state.clone(),
                created_at: None,
                started_at: None,
                finished_at: None,
                summary,
                steps,
                outputs: Default::default(),
                state_history: vec![],
            }),
        }
    }

    fn start(&self) -> Result<(), WorkRunError> {
        {
            let mut state = self.state.lock().unwrap();
            if state.run_state != WorkRunState::Pending {
                return Err(WorkRunError::AlreadyStarted);
            }
            state.run_state = WorkRunState::Running;
        }
        let _ = self.sender.send(WorkRunEvent::StateChanged(WorkRunState::Running));
        // Notify the orchestrator about steps that can start immediately.
        self.emit_ready_steps();
        Ok(())
    }

    fn cancel(&self) -> Result<(), WorkRunError> {
        let mut state = self.state.lock().unwrap();
        if matches!(state.run_state, WorkRunState::Succeeded | WorkRunState::Failed | WorkRunState::Cancelled) {
            return Err(WorkRunError::AlreadyFinished);
        }
        state.run_state = WorkRunState::Cancelled;
        for step_state in state.steps.values_mut() {
            if *step_state == WorkRunState::Pending || *step_state == WorkRunState::Running {
                *step_state = WorkRunState::Cancelled;
            }
        }
        let _ = self.sender.send(WorkRunEvent::StateChanged(WorkRunState::Cancelled));
        let _ = self.sender.send(WorkRunEvent::Finished);
        Ok(())
    }

    fn grant(&self, resources: WorkRunResources) -> Result<WorkRunResources, WorkRunError> {
        let state = self.state.lock().unwrap();
        if state.run_state != WorkRunState::Running {
            return Ok(resources); // return all unused when not running
        }
        // Count only steps that are pending AND have all dependencies succeeded.
        let ready_count = self
            .request
            .spec
            .steps
            .iter()
            .filter(|s| state.steps.get(&s.name) == Some(&WorkRunState::Pending))
            .filter(|s| {
                s.depends_on
                    .iter()
                    .all(|dep| state.steps.get(dep) == Some(&WorkRunState::Succeeded))
            })
            .count();
        let used = ready_count.min(resources.task_permits);
        Ok(WorkRunResources {
            task_permits: resources.task_permits - used,
        })
    }

    fn subscribe(&self) -> WorkRunEventStream {
        let rx = self.receiver.clone();
        let stream = futures_channel_adapter::CrossbeamStream::new(rx);
        Box::pin(stream)
    }

    fn on_task_run_update(&self, step_name: &str, task_run: TaskRunDoc) {
        if let Some(status) = &task_run.status {
            let new_state = task_run_state_to_work_run_state(&status.state);
            {
                let mut state = self.state.lock().unwrap();
                if let Some(step_state) = state.steps.get_mut(step_name) {
                    *step_state = new_state.clone();
                }
                if matches!(new_state, WorkRunState::Running) {
                    state.active_task_run_refs.insert(step_name.to_string(), task_run.name.clone());
                } else {
                    state.active_task_run_refs.remove(step_name);
                }
            }
            let _ = self.sender.send(WorkRunEvent::TaskRunUpdated {
                step_name: step_name.to_string(),
                task_run_ref: task_run.name.clone(),
                state: status.state.clone(),
            });
            let _ = self.sender.send(WorkRunEvent::StepStateChanged {
                step_name: step_name.to_string(),
                state: new_state.clone(),
            });
            // When a step succeeds, newly unblocked steps may become schedulable.
            if new_state == WorkRunState::Succeeded {
                self.emit_ready_steps();
            }
            // Check whether the entire run has completed.
            self.check_completion();
        }
    }
}

// ─── ThreadWorkRun helpers ────────────────────────────────────────────────────

impl ThreadWorkRun {
    /// Emit `StepStateChanged(Pending)` events for steps that are pending and
    /// whose every dependency has already succeeded — signalling to the
    /// orchestrator that those steps are ready to be scheduled.
    fn emit_ready_steps(&self) {
        let state = self.state.lock().unwrap();
        if state.run_state != WorkRunState::Running {
            return;
        }
        let ready: Vec<String> = self
            .request
            .spec
            .steps
            .iter()
            .filter(|s| state.steps.get(&s.name) == Some(&WorkRunState::Pending))
            .filter(|s| {
                s.depends_on
                    .iter()
                    .all(|dep| state.steps.get(dep) == Some(&WorkRunState::Succeeded))
            })
            .map(|s| s.name.clone())
            .collect();
        drop(state);
        for step_name in ready {
            let _ = self.sender.send(WorkRunEvent::StepStateChanged {
                step_name,
                state: WorkRunState::Pending,
            });
        }
    }

    /// Check whether all steps have reached a terminal state and, if so,
    /// transition the overall run and emit a `Finished` event.
    fn check_completion(&self) {
        let mut state = self.state.lock().unwrap();
        if state.run_state != WorkRunState::Running {
            return;
        }
        let all_terminal = state.steps.values().all(|s| {
            matches!(s, WorkRunState::Succeeded | WorkRunState::Failed | WorkRunState::Cancelled)
        });
        if !all_terminal {
            return;
        }
        let has_failed = state.steps.values().any(|s| *s == WorkRunState::Failed);
        let has_cancelled = state.steps.values().any(|s| *s == WorkRunState::Cancelled);
        let final_state = if has_failed {
            WorkRunState::Failed
        } else if has_cancelled {
            WorkRunState::Cancelled
        } else {
            WorkRunState::Succeeded
        };
        state.run_state = final_state.clone();
        drop(state);
        let _ = self.sender.send(WorkRunEvent::StateChanged(final_state));
        let _ = self.sender.send(WorkRunEvent::Finished);
    }
}

// ─── Helper ───────────────────────────────────────────────────────────────────

fn task_run_state_to_work_run_state(s: &workaholic::TaskRunState) -> WorkRunState {
    match s {
        workaholic::TaskRunState::Pending => WorkRunState::Pending,
        workaholic::TaskRunState::Running => WorkRunState::Running,
        workaholic::TaskRunState::Succeeded => WorkRunState::Succeeded,
        workaholic::TaskRunState::Failed => WorkRunState::Failed,
        workaholic::TaskRunState::Cancelled => WorkRunState::Cancelled,
    }
}

// ─── ThreadWorkRunHandle ───────────────────────────────────────────────────────

/// Owned wrapper returned by `ThreadWorkRunner::spawn()`.
#[derive(Debug)]
struct ThreadWorkRunHandle(Arc<ThreadWorkRun>);

impl WorkRun for ThreadWorkRunHandle {
    fn as_doc(&self) -> WorkRunDoc { self.0.as_doc() }
    fn start(&self) -> Result<(), WorkRunError> { self.0.start() }
    fn cancel(&self) -> Result<(), WorkRunError> { self.0.cancel() }
    fn grant(&self, r: WorkRunResources) -> Result<WorkRunResources, WorkRunError> { self.0.grant(r) }
    fn subscribe(&self) -> WorkRunEventStream { self.0.subscribe() }
    fn on_task_run_update(&self, step_name: &str, doc: TaskRunDoc) { self.0.on_task_run_update(step_name, doc) }
}

// ─── Stream adapter ────────────────────────────────────────────────────────────

/// Thin adapter that turns a `crossbeam_channel::Receiver` into a `futures_core::Stream`.
mod futures_channel_adapter {
    use std::pin::Pin;
    use std::task::{Context, Poll};

    use crossbeam_channel::Receiver;
    use futures_core::Stream;

    pub struct CrossbeamStream<T> {
        rx: Receiver<T>,
    }

    impl<T> CrossbeamStream<T> {
        pub fn new(rx: Receiver<T>) -> Self { Self { rx } }
    }

    impl<T: Unpin> Stream for CrossbeamStream<T> {
        type Item = T;

        fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<T>> {
            match self.rx.try_recv() {
                Ok(item) => Poll::Ready(Some(item)),
                Err(crossbeam_channel::TryRecvError::Empty) => {
                    // Register a waker wake so the executor will re-poll.
                    // For a synchronous channel we wake immediately; a proper
                    // async adapter would park the waker instead.
                    cx.waker().wake_by_ref();
                    Poll::Pending
                }
                Err(crossbeam_channel::TryRecvError::Disconnected) => Poll::Ready(None),
            }
        }
    }
}
