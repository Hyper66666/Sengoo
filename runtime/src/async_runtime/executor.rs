use std::collections::{HashMap, VecDeque};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, LazyLock, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use super::{
    bridge::DetachedForeignAsyncTask, clear_poll_wakeup_hint, take_poll_wakeup_hint, CoroutineTask,
    TaskId, TaskLifecycleStatus, TaskState,
};

const STATUS_INVALID_ARGUMENT: i64 = 2;
const IDLE_REPOLL_DELAY: Duration = Duration::from_millis(1);
const CONCURRENT_TASK_ID_TAG: i64 = 1_i64 << 62;
const RETIRED_STATUS_CAPACITY: usize = 4_096;

static EXECUTOR: Mutex<Option<ConcurrentExecutor>> = Mutex::new(None);
static NEXT_CONCURRENT_TASK_ID: AtomicU64 = AtomicU64::new(1);
static RETIRED_STATUSES: LazyLock<Mutex<RetiredTaskStatuses>> =
    LazyLock::new(|| Mutex::new(RetiredTaskStatuses::new(RETIRED_STATUS_CAPACITY)));
#[cfg(test)]
pub(crate) static EXECUTOR_TEST_GUARD: Mutex<()> = Mutex::new(());

struct RetiredTaskStatuses {
    capacity: usize,
    statuses: HashMap<TaskId, TaskLifecycleStatus>,
    order: VecDeque<TaskId>,
}

impl RetiredTaskStatuses {
    fn new(capacity: usize) -> Self {
        Self {
            capacity,
            statuses: HashMap::new(),
            order: VecDeque::new(),
        }
    }

    fn retire(&mut self, statuses: HashMap<TaskId, TaskLifecycleStatus>) {
        let mut statuses = statuses.into_iter().collect::<Vec<_>>();
        statuses.sort_unstable_by_key(|(task_id, _)| *task_id);
        for (task_id, status) in statuses {
            if status == TaskLifecycleStatus::Pending || status == TaskLifecycleStatus::Unknown {
                continue;
            }
            if self.statuses.insert(task_id, status).is_none() {
                self.order.push_back(task_id);
            }
            while self.order.len() > self.capacity {
                if let Some(expired) = self.order.pop_front() {
                    self.statuses.remove(&expired);
                }
            }
        }
    }

    fn status(&self, task_id: TaskId) -> TaskLifecycleStatus {
        self.statuses
            .get(&task_id)
            .copied()
            .unwrap_or(TaskLifecycleStatus::Unknown)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ConcurrentSubmitError {
    AtCapacity,
    ShuttingDown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ConcurrentShutdownMode {
    Drain,
    Cancel,
}

struct ConcurrentTask {
    id: TaskId,
    worker_index: usize,
    task: Mutex<Option<Box<dyn CoroutineTask + Send>>>,
    cancel_requested: AtomicBool,
}

struct QueuedTask {
    control: Arc<ConcurrentTask>,
    next_wakeup: Option<Instant>,
}

struct ExecutorState {
    queues: Vec<VecDeque<QueuedTask>>,
    next_worker: usize,
    controls: HashMap<TaskId, Arc<ConcurrentTask>>,
    statuses: HashMap<TaskId, TaskLifecycleStatus>,
    active: usize,
    accepting: bool,
    shutdown_mode: Option<ConcurrentShutdownMode>,
    stop_workers: bool,
}

struct ExecutorShared {
    capacity: usize,
    state: Mutex<ExecutorState>,
    worker_ready: Vec<Condvar>,
    task_finished: Condvar,
}

pub(crate) struct ConcurrentExecutor {
    shared: Arc<ExecutorShared>,
    workers: Vec<JoinHandle<()>>,
}

impl ConcurrentExecutor {
    pub(crate) fn new(worker_count: usize, capacity: usize) -> Result<Self, i64> {
        if worker_count == 0 || capacity == 0 {
            return Err(STATUS_INVALID_ARGUMENT);
        }

        let shared = Arc::new(ExecutorShared {
            capacity,
            state: Mutex::new(ExecutorState {
                queues: (0..worker_count).map(|_| VecDeque::new()).collect(),
                next_worker: 0,
                controls: HashMap::new(),
                statuses: HashMap::new(),
                active: 0,
                accepting: true,
                shutdown_mode: None,
                stop_workers: false,
            }),
            worker_ready: (0..worker_count).map(|_| Condvar::new()).collect(),
            task_finished: Condvar::new(),
        });
        let workers = (0..worker_count)
            .map(|worker_index| {
                let shared = shared.clone();
                thread::spawn(move || worker_loop(shared, worker_index))
            })
            .collect();
        Ok(Self { shared, workers })
    }

    #[cfg(test)]
    pub(crate) fn spawn<T>(&self, task: T) -> Result<TaskId, ConcurrentSubmitError>
    where
        T: CoroutineTask + Send + 'static,
    {
        self.spawn_returning_task(task).map_err(|(error, _)| error)
    }

    fn spawn_returning_task<T>(&self, task: T) -> Result<TaskId, (ConcurrentSubmitError, T)>
    where
        T: CoroutineTask + Send + 'static,
    {
        let mut state = self.shared.state.lock().expect("executor state poisoned");
        if !state.accepting {
            return Err((ConcurrentSubmitError::ShuttingDown, task));
        }
        if state.active >= self.shared.capacity {
            return Err((ConcurrentSubmitError::AtCapacity, task));
        }

        let id = NEXT_CONCURRENT_TASK_ID.fetch_add(1, Ordering::AcqRel);
        if id == 0 || id >= CONCURRENT_TASK_ID_TAG as u64 {
            return Err((ConcurrentSubmitError::ShuttingDown, task));
        }
        let worker_index = state.next_worker % state.queues.len();
        state.next_worker = state.next_worker.wrapping_add(1);
        let control = Arc::new(ConcurrentTask {
            id,
            worker_index,
            task: Mutex::new(Some(Box::new(task))),
            cancel_requested: AtomicBool::new(false),
        });
        state.queues[worker_index].push_back(QueuedTask {
            control: control.clone(),
            next_wakeup: None,
        });
        state.controls.insert(id, control);
        state.statuses.insert(id, TaskLifecycleStatus::Pending);
        state.active += 1;
        drop(state);
        self.shared.worker_ready[worker_index].notify_one();
        Ok(id)
    }

    #[cfg(test)]
    pub(crate) fn task_status(&self, task_id: TaskId) -> TaskLifecycleStatus {
        self.shared
            .state
            .lock()
            .expect("executor state poisoned")
            .statuses
            .get(&task_id)
            .copied()
            .unwrap_or(TaskLifecycleStatus::Unknown)
    }

    #[cfg(test)]
    pub(crate) fn cancel(&self, task_id: TaskId) -> bool {
        let state = self.shared.state.lock().expect("executor state poisoned");
        if state.statuses.get(&task_id) != Some(&TaskLifecycleStatus::Pending) {
            return false;
        }
        let Some(control) = state.controls.get(&task_id) else {
            return false;
        };
        control.cancel_requested.store(true, Ordering::Release);
        let worker_index = control.worker_index;
        drop(state);
        self.shared.worker_ready[worker_index].notify_one();
        true
    }

    #[cfg(test)]
    pub(crate) fn join(&self, task_id: TaskId) -> TaskLifecycleStatus {
        let mut state = self.shared.state.lock().expect("executor state poisoned");
        loop {
            let status = state
                .statuses
                .get(&task_id)
                .copied()
                .unwrap_or(TaskLifecycleStatus::Unknown);
            if status != TaskLifecycleStatus::Pending {
                return status;
            }
            state = self
                .shared
                .task_finished
                .wait(state)
                .expect("executor join wait poisoned");
        }
    }

    pub(crate) fn shutdown(&mut self, mode: ConcurrentShutdownMode) {
        if self.workers.is_empty() {
            return;
        }

        let mut state = self.shared.state.lock().expect("executor state poisoned");
        state.accepting = false;
        if mode == ConcurrentShutdownMode::Cancel
            || state.shutdown_mode == Some(ConcurrentShutdownMode::Cancel)
        {
            state.shutdown_mode = Some(ConcurrentShutdownMode::Cancel);
            for control in state.controls.values() {
                control.cancel_requested.store(true, Ordering::Release);
            }
        } else {
            state.shutdown_mode = Some(ConcurrentShutdownMode::Drain);
        }
        notify_all_workers(&self.shared);

        while state.active != 0 {
            state = self
                .shared
                .task_finished
                .wait(state)
                .expect("executor shutdown wait poisoned");
        }
        state.stop_workers = true;
        drop(state);
        notify_all_workers(&self.shared);

        for worker in self.workers.drain(..) {
            let _ = worker.join();
        }
    }
}

impl Drop for ConcurrentExecutor {
    fn drop(&mut self) {
        self.shutdown(ConcurrentShutdownMode::Cancel);
    }
}

pub(crate) fn enable(worker_count: i64, capacity: i64) -> Result<i64, i64> {
    let Ok(worker_count) = usize::try_from(worker_count) else {
        return Err(STATUS_INVALID_ARGUMENT);
    };
    let Ok(capacity) = usize::try_from(capacity) else {
        return Err(STATUS_INVALID_ARGUMENT);
    };
    if worker_count == 0 || capacity == 0 {
        return Err(STATUS_INVALID_ARGUMENT);
    }

    let mut executor = EXECUTOR.lock().expect("global executor mutex poisoned");
    if executor.is_none() {
        *executor = Some(ConcurrentExecutor::new(worker_count, capacity)?);
    }
    Ok(1)
}

pub(crate) fn is_enabled() -> bool {
    EXECUTOR
        .lock()
        .expect("global executor mutex poisoned")
        .is_some()
}

pub(crate) fn spawn_detached(kind: i64, handle: i64) -> i64 {
    let executor = EXECUTOR.lock().expect("global executor mutex poisoned");
    let Some(executor) = executor.as_ref() else {
        return 0;
    };
    match executor.spawn_returning_task(DetachedForeignAsyncTask { kind, handle }) {
        Ok(task_id) => encode_task_id(task_id).unwrap_or(0),
        Err((_, task)) => {
            task.release_rejected();
            0
        }
    }
}

pub(crate) fn is_concurrent_task_id(task_id: i64) -> bool {
    decode_task_id(task_id).is_some()
}

pub(crate) fn task_status(task_id: i64) -> TaskLifecycleStatus {
    let Some(task_id) = decode_task_id(task_id) else {
        return TaskLifecycleStatus::Unknown;
    };
    let shared = EXECUTOR
        .lock()
        .expect("global executor mutex poisoned")
        .as_ref()
        .map(|executor| executor.shared.clone());
    let active_status = shared
        .as_ref()
        .map_or(TaskLifecycleStatus::Unknown, |shared| {
            task_status_shared(shared, task_id)
        });
    if active_status != TaskLifecycleStatus::Unknown {
        return active_status;
    }
    RETIRED_STATUSES
        .lock()
        .expect("retired executor status mutex poisoned")
        .status(task_id)
}

pub(crate) fn cancel(task_id: i64) -> bool {
    let Some(task_id) = decode_task_id(task_id) else {
        return false;
    };
    let shared = EXECUTOR
        .lock()
        .expect("global executor mutex poisoned")
        .as_ref()
        .map(|executor| executor.shared.clone());
    shared
        .as_ref()
        .is_some_and(|shared| cancel_shared(shared, task_id))
}

pub(crate) fn join(task_id: i64) -> TaskLifecycleStatus {
    let Some(task_id) = decode_task_id(task_id) else {
        return TaskLifecycleStatus::Unknown;
    };
    let shared = EXECUTOR
        .lock()
        .expect("global executor mutex poisoned")
        .as_ref()
        .map(|executor| executor.shared.clone());
    if let Some(shared) = shared {
        let status = task_status_shared(&shared, task_id);
        if status != TaskLifecycleStatus::Unknown {
            return join_shared(&shared, task_id);
        }
    }
    RETIRED_STATUSES
        .lock()
        .expect("retired executor status mutex poisoned")
        .status(task_id)
}

pub(crate) fn shutdown(mode: ConcurrentShutdownMode) {
    let mut executor_slot = EXECUTOR.lock().expect("global executor mutex poisoned");
    if let Some(mut executor) = executor_slot.take() {
        executor.shutdown(mode);
        let statuses = executor
            .shared
            .state
            .lock()
            .expect("executor state poisoned")
            .statuses
            .clone();
        RETIRED_STATUSES
            .lock()
            .expect("retired executor status mutex poisoned")
            .retire(statuses);
    }
}

fn encode_task_id(task_id: TaskId) -> Option<i64> {
    let task_id = i64::try_from(task_id).ok()?;
    (task_id > 0 && task_id < CONCURRENT_TASK_ID_TAG).then_some(task_id | CONCURRENT_TASK_ID_TAG)
}

fn decode_task_id(task_id: i64) -> Option<TaskId> {
    if task_id <= CONCURRENT_TASK_ID_TAG || task_id & CONCURRENT_TASK_ID_TAG == 0 {
        return None;
    }
    Some((task_id & !CONCURRENT_TASK_ID_TAG) as TaskId)
}

fn task_status_shared(shared: &ExecutorShared, task_id: TaskId) -> TaskLifecycleStatus {
    shared
        .state
        .lock()
        .expect("executor state poisoned")
        .statuses
        .get(&task_id)
        .copied()
        .unwrap_or(TaskLifecycleStatus::Unknown)
}

fn cancel_shared(shared: &ExecutorShared, task_id: TaskId) -> bool {
    let state = shared.state.lock().expect("executor state poisoned");
    if state.statuses.get(&task_id) != Some(&TaskLifecycleStatus::Pending) {
        return false;
    }
    let Some(control) = state.controls.get(&task_id) else {
        return false;
    };
    control.cancel_requested.store(true, Ordering::Release);
    let worker_index = control.worker_index;
    drop(state);
    shared.worker_ready[worker_index].notify_one();
    true
}

fn notify_all_workers(shared: &ExecutorShared) {
    for ready in &shared.worker_ready {
        ready.notify_all();
    }
}

fn join_shared(shared: &ExecutorShared, task_id: TaskId) -> TaskLifecycleStatus {
    let mut state = shared.state.lock().expect("executor state poisoned");
    loop {
        let status = state
            .statuses
            .get(&task_id)
            .copied()
            .unwrap_or(TaskLifecycleStatus::Unknown);
        if status != TaskLifecycleStatus::Pending {
            return status;
        }
        state = shared
            .task_finished
            .wait(state)
            .expect("executor join wait poisoned");
    }
}

fn next_ready_task(
    state: &mut ExecutorState,
    worker_index: usize,
) -> (Option<QueuedTask>, Option<Duration>) {
    let now = Instant::now();
    let mut earliest = None;
    let Some(queue) = state.queues.get_mut(worker_index) else {
        return (None, None);
    };
    let queued = queue.len();
    for _ in 0..queued {
        let Some(task) = queue.pop_front() else {
            break;
        };
        if task.next_wakeup.is_none_or(|deadline| deadline <= now)
            || task.control.cancel_requested.load(Ordering::Acquire)
        {
            return (Some(task), None);
        }
        let deadline = task.next_wakeup.expect("checked wakeup deadline");
        let delay = deadline.saturating_duration_since(now);
        earliest = Some(earliest.map_or(delay, |current: Duration| current.min(delay)));
        queue.push_back(task);
    }
    (None, earliest)
}

fn worker_loop(shared: Arc<ExecutorShared>, worker_index: usize) {
    loop {
        let queued = {
            let mut state = shared.state.lock().expect("executor state poisoned");
            loop {
                if state.stop_workers {
                    return;
                }
                let (task, delay) = next_ready_task(&mut state, worker_index);
                if let Some(task) = task {
                    break task;
                }
                state = if let Some(delay) = delay {
                    shared.worker_ready[worker_index]
                        .wait_timeout(state, delay.min(IDLE_REPOLL_DELAY))
                        .expect("executor timed wait poisoned")
                        .0
                } else {
                    shared.worker_ready[worker_index]
                        .wait(state)
                        .expect("executor work wait poisoned")
                };
            }
        };
        poll_task(&shared, queued);
    }
}

fn poll_task(shared: &ExecutorShared, queued: QueuedTask) {
    let control = queued.control;
    let Some(mut task) = control
        .task
        .lock()
        .expect("executor task state poisoned")
        .take()
    else {
        return;
    };

    if control.cancel_requested.load(Ordering::Acquire) {
        cancel_task(shared, &control, task.as_mut());
        return;
    }

    clear_poll_wakeup_hint();
    let outcome = catch_unwind(AssertUnwindSafe(|| task.poll()));
    match outcome {
        Ok(TaskState::Complete) => {
            finish_task(shared, control.id, TaskLifecycleStatus::Completed);
        }
        Ok(TaskState::Pending) => {
            if control.cancel_requested.load(Ordering::Acquire) {
                cancel_task(shared, &control, task.as_mut());
                return;
            }
            let wakeup =
                take_poll_wakeup_hint().or_else(|| Some(Instant::now() + IDLE_REPOLL_DELAY));
            *control.task.lock().expect("executor task state poisoned") = Some(task);
            let mut state = shared.state.lock().expect("executor state poisoned");
            let worker_index = control.worker_index;
            state.queues[worker_index].push_back(QueuedTask {
                control,
                next_wakeup: wakeup,
            });
            drop(state);
            shared.worker_ready[worker_index].notify_one();
        }
        Err(_) => {
            let _ = catch_unwind(AssertUnwindSafe(|| task.on_scheduler_drop()));
            finish_task(shared, control.id, TaskLifecycleStatus::Failed);
        }
    }
}

fn cancel_task(shared: &ExecutorShared, control: &ConcurrentTask, task: &mut dyn CoroutineTask) {
    let cancel_result = catch_unwind(AssertUnwindSafe(|| task.cancel()));
    let status = match cancel_result {
        Ok(true) => TaskLifecycleStatus::Canceled,
        Ok(false) => {
            if catch_unwind(AssertUnwindSafe(|| task.on_scheduler_drop())).is_ok() {
                TaskLifecycleStatus::Canceled
            } else {
                TaskLifecycleStatus::Failed
            }
        }
        Err(_) => {
            let _ = catch_unwind(AssertUnwindSafe(|| task.on_scheduler_drop()));
            TaskLifecycleStatus::Failed
        }
    };
    finish_task(shared, control.id, status);
}

fn finish_task(shared: &ExecutorShared, task_id: TaskId, status: TaskLifecycleStatus) {
    let mut state = shared.state.lock().expect("executor state poisoned");
    if state.statuses.get(&task_id) != Some(&TaskLifecycleStatus::Pending) {
        return;
    }
    state.statuses.insert(task_id, status);
    state.controls.remove(&task_id);
    state.active = state.active.saturating_sub(1);
    drop(state);
    shared.task_finished.notify_all();
    notify_all_workers(shared);
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Barrier, Condvar, Mutex};

    use super::{
        ConcurrentExecutor, ConcurrentShutdownMode, ConcurrentSubmitError, RetiredTaskStatuses,
    };
    use crate::async_runtime::{CoroutineTask, TaskLifecycleStatus, TaskState};

    struct ReadyTask;

    impl CoroutineTask for ReadyTask {
        fn poll(&mut self) -> TaskState {
            TaskState::Complete
        }
    }

    #[test]
    fn retired_executor_statuses_are_bounded_and_keep_newest_task_ids() {
        let mut retired = RetiredTaskStatuses::new(2);
        retired.retire(HashMap::from([
            (3, TaskLifecycleStatus::Completed),
            (1, TaskLifecycleStatus::Canceled),
            (2, TaskLifecycleStatus::Failed),
        ]));

        assert_eq!(retired.status(1), TaskLifecycleStatus::Unknown);
        assert_eq!(retired.status(2), TaskLifecycleStatus::Failed);
        assert_eq!(retired.status(3), TaskLifecycleStatus::Completed);
    }

    struct BlockingTask {
        release: Arc<(Mutex<bool>, Condvar)>,
    }

    impl CoroutineTask for BlockingTask {
        fn poll(&mut self) -> TaskState {
            let (lock, wake) = &*self.release;
            let mut released = lock.lock().expect("release mutex poisoned");
            while !*released {
                released = wake.wait(released).expect("release wait poisoned");
            }
            TaskState::Complete
        }
    }

    #[test]
    fn bounded_executor_rejects_submission_at_capacity_and_recovers_after_join() {
        let release = Arc::new((Mutex::new(false), Condvar::new()));
        let mut executor = ConcurrentExecutor::new(1, 2).expect("valid executor config");
        let first = executor
            .spawn(BlockingTask {
                release: release.clone(),
            })
            .expect("first task should be accepted");
        let second = executor
            .spawn(ReadyTask)
            .expect("queue slot should be accepted");

        assert_eq!(
            executor.spawn(ReadyTask),
            Err(ConcurrentSubmitError::AtCapacity)
        );

        let (lock, wake) = &*release;
        *lock.lock().expect("release mutex poisoned") = true;
        wake.notify_all();
        assert_eq!(executor.join(first), TaskLifecycleStatus::Completed);
        assert_eq!(executor.join(second), TaskLifecycleStatus::Completed);

        let third = executor
            .spawn(ReadyTask)
            .expect("capacity should be returned after completion");
        assert_eq!(executor.join(third), TaskLifecycleStatus::Completed);
        executor.shutdown(ConcurrentShutdownMode::Drain);
    }

    struct ParallelTask {
        barrier: Arc<Barrier>,
        completed: Arc<AtomicUsize>,
    }

    impl CoroutineTask for ParallelTask {
        fn poll(&mut self) -> TaskState {
            self.barrier.wait();
            self.completed.fetch_add(1, Ordering::AcqRel);
            TaskState::Complete
        }
    }

    #[test]
    fn bounded_executor_runs_send_tasks_in_parallel_and_joins_by_task_id() {
        let barrier = Arc::new(Barrier::new(2));
        let completed = Arc::new(AtomicUsize::new(0));
        let mut executor = ConcurrentExecutor::new(2, 4).expect("valid executor config");
        let first = executor
            .spawn(ParallelTask {
                barrier: barrier.clone(),
                completed: completed.clone(),
            })
            .expect("first task should be accepted");
        let second = executor
            .spawn(ParallelTask {
                barrier,
                completed: completed.clone(),
            })
            .expect("second task should be accepted");

        assert_eq!(executor.join(first), TaskLifecycleStatus::Completed);
        assert_eq!(executor.join(second), TaskLifecycleStatus::Completed);
        assert_eq!(completed.load(Ordering::Acquire), 2);
        executor.shutdown(ConcurrentShutdownMode::Drain);
    }

    struct PendingTask {
        canceled: Arc<AtomicUsize>,
    }

    impl CoroutineTask for PendingTask {
        fn poll(&mut self) -> TaskState {
            TaskState::Pending
        }

        fn cancel(&mut self) -> bool {
            self.canceled.fetch_add(1, Ordering::AcqRel);
            true
        }
    }

    #[test]
    fn bounded_executor_cancel_and_shutdown_do_not_leak_pending_tasks() {
        let canceled = Arc::new(AtomicUsize::new(0));
        let mut executor = ConcurrentExecutor::new(2, 4).expect("valid executor config");
        let explicit = executor
            .spawn(PendingTask {
                canceled: canceled.clone(),
            })
            .expect("explicitly canceled task should be accepted");
        let shutdown = executor
            .spawn(PendingTask {
                canceled: canceled.clone(),
            })
            .expect("shutdown-canceled task should be accepted");

        assert!(executor.cancel(explicit));
        assert_eq!(executor.join(explicit), TaskLifecycleStatus::Canceled);
        executor.shutdown(ConcurrentShutdownMode::Cancel);
        assert_eq!(
            executor.task_status(shutdown),
            TaskLifecycleStatus::Canceled
        );
        assert_eq!(canceled.load(Ordering::Acquire), 2);
        assert_eq!(
            executor.spawn(ReadyTask),
            Err(ConcurrentSubmitError::ShuttingDown)
        );
    }

    struct PanicTask;

    impl CoroutineTask for PanicTask {
        fn poll(&mut self) -> TaskState {
            panic!("intentional executor test panic")
        }
    }

    #[test]
    fn bounded_executor_isolates_task_panics_from_other_workers() {
        let mut executor = ConcurrentExecutor::new(2, 4).expect("valid executor config");
        let failed = executor
            .spawn(PanicTask)
            .expect("panic task should be accepted");
        let healthy = executor
            .spawn(ReadyTask)
            .expect("healthy task should be accepted");

        assert_eq!(executor.join(failed), TaskLifecycleStatus::Failed);
        assert_eq!(executor.join(healthy), TaskLifecycleStatus::Completed);
        executor.shutdown(ConcurrentShutdownMode::Drain);
    }

    struct PanicCleanupTask;

    impl CoroutineTask for PanicCleanupTask {
        fn poll(&mut self) -> TaskState {
            panic!("intentional poll panic before cleanup")
        }

        fn on_scheduler_drop(&mut self) {
            panic!("intentional cleanup panic")
        }
    }

    #[test]
    fn bounded_executor_contains_cleanup_panics_and_releases_capacity() {
        let mut executor = ConcurrentExecutor::new(1, 1).expect("valid executor config");
        let failed = executor
            .spawn(PanicCleanupTask)
            .expect("panic cleanup task should be accepted");

        let deadline = std::time::Instant::now() + std::time::Duration::from_millis(100);
        while executor.task_status(failed) == TaskLifecycleStatus::Pending
            && std::time::Instant::now() < deadline
        {
            std::thread::yield_now();
        }
        let status = executor.task_status(failed);
        if status == TaskLifecycleStatus::Pending {
            std::mem::forget(executor);
            panic!("cleanup panic stranded the executor task");
        }
        assert_eq!(status, TaskLifecycleStatus::Failed);
        let healthy = executor
            .spawn(ReadyTask)
            .expect("failed cleanup must release capacity");
        assert_eq!(executor.join(healthy), TaskLifecycleStatus::Completed);
        executor.shutdown(ConcurrentShutdownMode::Drain);
    }

    #[test]
    fn global_executor_tags_foreign_tasks_and_exposes_join_status() {
        let _guard = super::EXECUTOR_TEST_GUARD
            .lock()
            .expect("executor test guard mutex poisoned");
        super::shutdown(ConcurrentShutdownMode::Cancel);
        assert_eq!(super::enable(2, 4), Ok(1));
        assert!(super::is_enabled());

        let task_id = super::spawn_detached(0, 0);
        assert!(super::is_concurrent_task_id(task_id));
        assert_eq!(super::join(task_id), TaskLifecycleStatus::Completed);
        assert_eq!(super::task_status(task_id), TaskLifecycleStatus::Completed);
        assert!(!super::cancel(task_id));

        super::shutdown(ConcurrentShutdownMode::Drain);
        assert!(!super::is_enabled());
        assert_eq!(super::join(task_id), TaskLifecycleStatus::Completed);
        assert_eq!(super::task_status(task_id), TaskLifecycleStatus::Completed);

        assert_eq!(super::enable(1, 1), Ok(1));
        let next_task_id = super::spawn_detached(0, 0);
        assert_ne!(next_task_id, task_id);
        assert_eq!(super::join(next_task_id), TaskLifecycleStatus::Completed);
        assert_eq!(super::task_status(task_id), TaskLifecycleStatus::Completed);
        super::shutdown(ConcurrentShutdownMode::Drain);
    }
}
