//! Coroutine-compatible async runtime scheduling primitives.

#[cfg(feature = "native-bridge")]
use std::cell::Cell;
use std::collections::{HashMap, VecDeque};
#[cfg(feature = "native-bridge")]
use std::ptr::NonNull;
#[cfg(feature = "native-bridge")]
use std::sync::{Arc, Mutex};
#[cfg(feature = "native-bridge")]
use std::time::Duration;
use std::time::Instant;

pub type TaskId = u64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskState {
    Pending,
    Complete,
}

#[repr(i64)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskLifecycleStatus {
    Unknown = 0,
    Pending = 1,
    Completed = 2,
    Canceled = 3,
}

pub trait CoroutineTask {
    fn poll(&mut self) -> TaskState;

    fn cancel(&mut self) -> bool {
        true
    }

    fn on_scheduler_drop(&mut self) {}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SchedulerStats {
    pub scheduled: usize,
    pub completed: usize,
}

struct TaskEntry {
    id: TaskId,
    task: Box<dyn CoroutineTask + Send>,
    next_wakeup: Option<Instant>,
}

/// Cooperative, tick-driven scheduler for Sengoo async tasks.
pub struct CoroutineScheduler {
    next_id: TaskId,
    queue: VecDeque<TaskEntry>,
    completed: usize,
    statuses: HashMap<TaskId, TaskLifecycleStatus>,
}

impl Default for CoroutineScheduler {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for CoroutineScheduler {
    fn drop(&mut self) {
        for mut entry in self.queue.drain(..) {
            entry.task.on_scheduler_drop();
        }
    }
}

impl CoroutineScheduler {
    pub fn new() -> Self {
        Self {
            next_id: 1,
            queue: VecDeque::new(),
            completed: 0,
            statuses: HashMap::new(),
        }
    }

    pub fn spawn<T>(&mut self, task: T) -> TaskId
    where
        T: CoroutineTask + Send + 'static,
    {
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        self.queue.push_back(TaskEntry {
            id,
            task: Box::new(task),
            next_wakeup: None,
        });
        self.statuses.insert(id, TaskLifecycleStatus::Pending);
        id
    }

    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    pub fn stats(&self) -> SchedulerStats {
        SchedulerStats {
            scheduled: self.queue.len(),
            completed: self.completed,
        }
    }

    pub fn task_status(&self, task_id: TaskId) -> TaskLifecycleStatus {
        self.statuses
            .get(&task_id)
            .copied()
            .unwrap_or(TaskLifecycleStatus::Unknown)
    }

    pub fn cancel(&mut self, task_id: TaskId) -> bool {
        let Some(index) = self.queue.iter().position(|entry| entry.id == task_id) else {
            return false;
        };

        let canceled = self
            .queue
            .get_mut(index)
            .map(|entry| entry.task.cancel())
            .unwrap_or(false);
        if !canceled {
            return false;
        }

        let _entry = self
            .queue
            .remove(index)
            .expect("task index located in queue should remain valid");
        self.statuses.insert(task_id, TaskLifecycleStatus::Canceled);
        true
    }

    /// Poll every queued task once.
    /// Completed tasks are removed and their IDs are returned.
    pub fn tick(&mut self) -> Vec<TaskId> {
        let mut ready = Vec::new();
        let cycle_len = self.queue.len();
        let now = Instant::now();

        for _ in 0..cycle_len {
            let Some(mut entry) = self.queue.pop_front() else {
                break;
            };

            if entry.next_wakeup.is_some_and(|deadline| deadline > now) {
                self.queue.push_back(entry);
                continue;
            }

            clear_poll_wakeup_hint();
            match entry.task.poll() {
                TaskState::Pending => {
                    entry.next_wakeup = take_poll_wakeup_hint();
                    self.queue.push_back(entry);
                }
                TaskState::Complete => {
                    self.completed += 1;
                    self.statuses
                        .insert(entry.id, TaskLifecycleStatus::Completed);
                    ready.push(entry.id);
                }
            }
        }

        ready
    }

    /// Run ticks until queue is empty or max_ticks is reached.
    pub fn run_until_idle(&mut self, max_ticks: usize) -> Vec<TaskId> {
        let mut finished = Vec::new();

        for _ in 0..max_ticks {
            if self.queue.is_empty() {
                break;
            }
            finished.extend(self.tick());
            if self.queue.is_empty() {
                break;
            }
            self.sleep_until_next_wakeup();
        }

        finished
    }

    fn sleep_until_next_wakeup(&self) {
        let now = Instant::now();
        let mut earliest = None;

        for entry in &self.queue {
            let Some(deadline) = entry.next_wakeup else {
                return;
            };
            if deadline <= now {
                return;
            }
            earliest = Some(match earliest {
                Some(current) if current <= deadline => current,
                _ => deadline,
            });
        }

        if let Some(deadline) = earliest {
            std::thread::sleep(deadline.saturating_duration_since(now));
        }
    }
}

#[cfg(feature = "native-bridge")]
unsafe extern "C" {
    fn main__start() -> i64;
    fn main__poll(handle: i64) -> i64;
    fn main__result(handle: i64) -> i64;
    fn sengoo_async_poll_dispatch(kind: i64, handle: i64) -> i64;
    fn sengoo_async_cancel_dispatch(kind: i64, handle: i64) -> bool;
    fn sengoo_async_drop_dispatch(kind: i64, handle: i64);
    fn sengoo_async_result_dispatch_i8(kind: i64, handle: i64) -> i8;
    fn sengoo_async_result_dispatch_i16(kind: i64, handle: i64) -> i16;
    fn sengoo_async_result_dispatch_i32(kind: i64, handle: i64) -> i32;
    fn sengoo_async_result_dispatch_i64(kind: i64, handle: i64) -> i64;
    fn sengoo_async_result_dispatch_bool(kind: i64, handle: i64) -> bool;
    fn sengoo_async_result_dispatch_f32(kind: i64, handle: i64) -> f32;
    fn sengoo_async_result_dispatch_f64(kind: i64, handle: i64) -> f64;
}

#[cfg(feature = "native-bridge")]
thread_local! {
    static CURRENT_SCHEDULER: Cell<*mut CoroutineScheduler> = const { Cell::new(std::ptr::null_mut()) };
    static POLL_WAKEUP_HINT: Cell<Option<Instant>> = const { Cell::new(None) };
}

#[cfg(feature = "native-bridge")]
fn clear_poll_wakeup_hint() {
    POLL_WAKEUP_HINT.with(|cell| cell.set(None));
}

#[cfg(not(feature = "native-bridge"))]
fn clear_poll_wakeup_hint() {}

#[cfg(feature = "native-bridge")]
fn record_poll_wakeup_hint(deadline: Instant) {
    POLL_WAKEUP_HINT.with(|cell| {
        let next = match cell.get() {
            Some(current) if current <= deadline => current,
            _ => deadline,
        };
        cell.set(Some(next));
    });
}

#[cfg(not(feature = "native-bridge"))]
#[allow(dead_code)]
fn record_poll_wakeup_hint(_deadline: Instant) {}

#[cfg(feature = "native-bridge")]
fn take_poll_wakeup_hint() -> Option<Instant> {
    POLL_WAKEUP_HINT.with(|cell| {
        let hint = cell.get();
        cell.set(None);
        hint
    })
}

#[cfg(not(feature = "native-bridge"))]
fn take_poll_wakeup_hint() -> Option<Instant> {
    None
}

#[cfg(feature = "native-bridge")]
fn merge_wakeup_hints(first: Option<Instant>, second: Option<Instant>) -> Option<Instant> {
    match (first, second) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    }
}

#[cfg(feature = "native-bridge")]
fn merge_wakeup_hint_with_deadline(hint: Option<Instant>, deadline: Instant) -> Instant {
    merge_wakeup_hints(hint, Some(deadline)).expect("deadline should always produce a wakeup hint")
}

#[cfg(feature = "native-bridge")]
fn wait_for_wakeup_hint_or_yield(deadline: Option<Instant>) {
    match deadline {
        Some(deadline) => {
            let now = Instant::now();
            let sleep_for = deadline.saturating_duration_since(now);
            if sleep_for.is_zero() {
                std::thread::yield_now();
            } else {
                std::thread::sleep(sleep_for);
            }
        }
        None => std::thread::yield_now(),
    }
}

#[cfg(feature = "native-bridge")]
const SELECT_WINNER_FIRST: i64 = 0;

#[cfg(feature = "native-bridge")]
const SELECT_WINNER_SECOND: i64 = 1;

#[cfg(feature = "native-bridge")]
unsafe fn wait_for_first_ready_winner(
    first_kind: i64,
    first_handle: i64,
    second_kind: i64,
    second_handle: i64,
) -> i64 {
    loop {
        clear_poll_wakeup_hint();
        if sengoo_async_poll_dispatch(first_kind, first_handle) != 0 {
            return SELECT_WINNER_FIRST;
        }
        let first_hint = take_poll_wakeup_hint();

        clear_poll_wakeup_hint();
        if sengoo_async_poll_dispatch(second_kind, second_handle) != 0 {
            return SELECT_WINNER_SECOND;
        }
        let second_hint = take_poll_wakeup_hint();

        wait_for_wakeup_hint_or_yield(merge_wakeup_hints(first_hint, second_hint));
    }
}

#[cfg(feature = "native-bridge")]
unsafe fn wait_for_first_ready<T>(
    first_kind: i64,
    first_handle: i64,
    second_kind: i64,
    second_handle: i64,
    dispatch: unsafe extern "C" fn(i64, i64) -> T,
) -> T {
    match wait_for_first_ready_winner(first_kind, first_handle, second_kind, second_handle) {
        SELECT_WINNER_FIRST => dispatch(first_kind, first_handle),
        SELECT_WINNER_SECOND => dispatch(second_kind, second_handle),
        winner => unreachable!("unexpected async select winner: {winner}"),
    }
}

#[cfg(feature = "native-bridge")]
/// Async runtime FFI contract:
/// - handle values come only from the matching `__start`/constructor function
/// - `0` is reserved as an invalid handle and must be treated as absent
/// - non-zero handles must point to a live allocation of the exact state type
/// - `__result` / `*_free` consume ownership exactly once for owning handle types
/// - callers must not alias mutable access across FFI boundaries
unsafe fn handle_nonnull<T>(handle: i64) -> Option<NonNull<T>> {
    NonNull::new(handle as *mut T)
}

#[cfg(feature = "native-bridge")]
unsafe fn handle_ref<'a, T>(handle: i64) -> Option<&'a T> {
    handle_nonnull(handle).map(|ptr| ptr.as_ref())
}

#[cfg(feature = "native-bridge")]
unsafe fn handle_mut<'a, T>(handle: i64) -> Option<&'a mut T> {
    handle_nonnull(handle).map(|mut ptr| ptr.as_mut())
}

#[cfg(feature = "native-bridge")]
unsafe fn handle_take_box<T>(handle: i64) -> Option<Box<T>> {
    handle_nonnull(handle).map(|ptr| Box::from_raw(ptr.as_ptr()))
}

#[cfg(feature = "native-bridge")]
fn scheduler_nonnull(ptr: *mut CoroutineScheduler) -> Option<NonNull<CoroutineScheduler>> {
    NonNull::new(ptr)
}

#[cfg(feature = "native-bridge")]
unsafe fn scheduler_mut<'a>(ptr: *mut CoroutineScheduler) -> Option<&'a mut CoroutineScheduler> {
    scheduler_nonnull(ptr).map(|mut ptr| ptr.as_mut())
}

#[cfg(feature = "native-bridge")]
struct RootAsyncMainI64Task {
    handle: Option<i64>,
    result: Arc<Mutex<Option<i64>>>,
}

#[cfg(feature = "native-bridge")]
impl RootAsyncMainI64Task {
    fn new(result: Arc<Mutex<Option<i64>>>) -> Self {
        Self {
            handle: None,
            result,
        }
    }
}

#[cfg(feature = "native-bridge")]
impl CoroutineTask for RootAsyncMainI64Task {
    fn poll(&mut self) -> TaskState {
        let handle = *self.handle.get_or_insert_with(|| unsafe { main__start() });
        if unsafe { main__poll(handle) } == 0 {
            return TaskState::Pending;
        }

        let result = unsafe { main__result(handle) };
        *self
            .result
            .lock()
            .expect("async main result mutex poisoned") = Some(result);
        TaskState::Complete
    }

    fn cancel(&mut self) -> bool {
        false
    }
}

#[cfg(feature = "native-bridge")]
struct ForeignAsyncTask {
    kind: i64,
    handle: i64,
}

#[cfg(feature = "native-bridge")]
impl CoroutineTask for ForeignAsyncTask {
    fn poll(&mut self) -> TaskState {
        if unsafe { sengoo_async_poll_dispatch(self.kind, self.handle) } == 0 {
            TaskState::Pending
        } else {
            TaskState::Complete
        }
    }

    fn cancel(&mut self) -> bool {
        if self.handle == 0 {
            return true;
        }
        let canceled = unsafe { sengoo_async_cancel_dispatch(self.kind, self.handle) };
        if canceled {
            self.handle = 0;
        }
        canceled
    }

    fn on_scheduler_drop(&mut self) {
        if self.handle != 0 {
            unsafe { sengoo_async_drop_dispatch(self.kind, self.handle) };
            self.handle = 0;
        }
    }
}

#[cfg(feature = "native-bridge")]
struct SleepFutureState {
    deadline: Instant,
}

#[cfg(feature = "native-bridge")]
struct TimeoutBoolFutureState {
    child_kind: i64,
    child_handle: i64,
    deadline: Instant,
    result: Option<bool>,
}

#[cfg(feature = "native-bridge")]
fn sleep_duration(duration_ms: i64) -> Duration {
    Duration::from_millis(duration_ms.max(0) as u64)
}

#[cfg(feature = "native-bridge")]
#[no_mangle]
pub extern "C" fn sengoo_async_sleep__start(duration_ms: i64) -> i64 {
    let state = SleepFutureState {
        deadline: Instant::now() + sleep_duration(duration_ms),
    };
    Box::into_raw(Box::new(state)) as i64
}

#[cfg(feature = "native-bridge")]
#[no_mangle]
pub unsafe extern "C" fn sengoo_async_sleep__poll(handle: i64) -> i64 {
    let Some(state) = handle_ref::<SleepFutureState>(handle) else {
        return 1;
    };
    if Instant::now() >= state.deadline {
        1
    } else {
        record_poll_wakeup_hint(state.deadline);
        0
    }
}

#[cfg(feature = "native-bridge")]
#[no_mangle]
pub unsafe extern "C" fn sengoo_async_sleep__result(handle: i64) {
    let Some(state) = handle_take_box::<SleepFutureState>(handle) else {
        return;
    };
    drop(state);
}

#[cfg(feature = "native-bridge")]
#[no_mangle]
pub unsafe extern "C" fn sengoo_async_sleep__cancel(handle: i64) -> bool {
    let Some(state) = handle_take_box::<SleepFutureState>(handle) else {
        return false;
    };
    drop(state);
    true
}

#[cfg(feature = "native-bridge")]
#[no_mangle]
pub unsafe extern "C" fn sengoo_async_sleep__drop(handle: i64) {
    let Some(state) = handle_take_box::<SleepFutureState>(handle) else {
        return;
    };
    drop(state);
}

#[cfg(feature = "native-bridge")]
#[no_mangle]
pub extern "C" fn sengoo_async_timeout_bool__start(
    child_kind: i64,
    child_handle: i64,
    duration_ms: i64,
) -> i64 {
    let state = TimeoutBoolFutureState {
        child_kind,
        child_handle,
        deadline: Instant::now() + sleep_duration(duration_ms),
        result: None,
    };
    Box::into_raw(Box::new(state)) as i64
}

#[cfg(feature = "native-bridge")]
#[no_mangle]
pub unsafe extern "C" fn sengoo_async_timeout_bool__poll(handle: i64) -> i64 {
    let Some(state) = handle_mut::<TimeoutBoolFutureState>(handle) else {
        return 1;
    };
    if state.result.is_some() {
        return 1;
    }

    clear_poll_wakeup_hint();
    if sengoo_async_poll_dispatch(state.child_kind, state.child_handle) != 0 {
        state.result = Some(true);
        return 1;
    }
    let child_hint = take_poll_wakeup_hint();

    let now = Instant::now();
    if now >= state.deadline {
        state.result = Some(false);
        return 1;
    }

    record_poll_wakeup_hint(merge_wakeup_hint_with_deadline(child_hint, state.deadline));
    0
}

#[cfg(feature = "native-bridge")]
#[no_mangle]
pub unsafe extern "C" fn sengoo_async_timeout_bool__result(handle: i64) -> bool {
    let Some(state) = handle_take_box::<TimeoutBoolFutureState>(handle) else {
        return false;
    };
    state.result.unwrap_or(false)
}

#[cfg(feature = "native-bridge")]
#[no_mangle]
pub unsafe extern "C" fn sengoo_async_timeout_bool__cancel(handle: i64) -> bool {
    let Some(state) = handle_take_box::<TimeoutBoolFutureState>(handle) else {
        return false;
    };
    drop(state);
    true
}

#[cfg(feature = "native-bridge")]
#[no_mangle]
pub unsafe extern "C" fn sengoo_async_timeout_bool__drop(handle: i64) {
    let Some(state) = handle_take_box::<TimeoutBoolFutureState>(handle) else {
        return;
    };
    drop(state);
}

#[cfg(feature = "native-bridge")]
#[no_mangle]
pub extern "C" fn sengoo_async_scheduler_new() -> *mut CoroutineScheduler {
    Box::into_raw(Box::new(CoroutineScheduler::new()))
}

#[cfg(feature = "native-bridge")]
#[no_mangle]
pub unsafe extern "C" fn sengoo_async_scheduler_free(scheduler: *mut CoroutineScheduler) {
    let Some(scheduler) = scheduler_nonnull(scheduler) else {
        return;
    };
    drop(Box::from_raw(scheduler.as_ptr()));
}

#[cfg(feature = "native-bridge")]
#[no_mangle]
pub unsafe extern "C" fn sengoo_async_scheduler_run_until_idle(
    scheduler: *mut CoroutineScheduler,
    max_ticks: usize,
) -> usize {
    let Some(scheduler_ref) = scheduler_mut(scheduler) else {
        return 0;
    };
    CURRENT_SCHEDULER.with(|cell| {
        let previous = cell.replace(scheduler);
        let finished = scheduler_ref.run_until_idle(max_ticks).len();
        cell.set(previous);
        finished
    })
}

#[cfg(feature = "native-bridge")]
#[no_mangle]
pub unsafe extern "C" fn sengoo_async_scheduler_cancel(
    scheduler: *mut CoroutineScheduler,
    task_id: i64,
) -> bool {
    let Some(scheduler_ref) = scheduler_mut(scheduler) else {
        return false;
    };
    if task_id <= 0 {
        return false;
    }
    scheduler_ref.cancel(task_id as TaskId)
}

#[cfg(feature = "native-bridge")]
#[no_mangle]
pub unsafe extern "C" fn sengoo_async_scheduler_task_status(
    scheduler: *mut CoroutineScheduler,
    task_id: i64,
) -> i64 {
    let Some(scheduler_ref) = scheduler_mut(scheduler) else {
        return TaskLifecycleStatus::Unknown as i64;
    };
    if task_id <= 0 {
        return TaskLifecycleStatus::Unknown as i64;
    }
    scheduler_ref.task_status(task_id as TaskId) as i64
}

#[cfg(feature = "native-bridge")]
#[no_mangle]
pub extern "C" fn sengoo_async_cancel_task(task_id: i64) -> bool {
    CURRENT_SCHEDULER.with(|cell| {
        let scheduler = cell.get();
        let Some(scheduler) = (unsafe { scheduler_mut(scheduler) }) else {
            return false;
        };
        if task_id <= 0 {
            return false;
        }
        scheduler.cancel(task_id as TaskId)
    })
}

#[cfg(feature = "native-bridge")]
#[no_mangle]
pub extern "C" fn sengoo_async_task_status(task_id: i64) -> i64 {
    CURRENT_SCHEDULER.with(|cell| {
        let scheduler = cell.get();
        let Some(scheduler) = (unsafe { scheduler_mut(scheduler) }) else {
            return TaskLifecycleStatus::Unknown as i64;
        };
        if task_id <= 0 {
            return TaskLifecycleStatus::Unknown as i64;
        }
        scheduler.task_status(task_id as TaskId) as i64
    })
}

#[cfg(feature = "native-bridge")]
#[no_mangle]
pub extern "C" fn sengoo_async_spawn_raw(kind: i64, handle: i64) -> i64 {
    CURRENT_SCHEDULER.with(|cell| {
        let scheduler = cell.get();
        let Some(scheduler) = (unsafe { scheduler_mut(scheduler) }) else {
            return 0;
        };
        scheduler.spawn(ForeignAsyncTask { kind, handle }) as i64
    })
}

#[cfg(feature = "native-bridge")]
macro_rules! define_async_select {
    ($name:ident, $dispatch:path, $ret:ty) => {
        #[cfg(feature = "native-bridge")]
        #[no_mangle]
        pub extern "C" fn $name(
            first_kind: i64,
            first_handle: i64,
            second_kind: i64,
            second_handle: i64,
        ) -> $ret {
            unsafe {
                wait_for_first_ready(
                    first_kind,
                    first_handle,
                    second_kind,
                    second_handle,
                    $dispatch,
                )
            }
        }
    };
}

#[cfg(feature = "native-bridge")]
#[no_mangle]
pub extern "C" fn sengoo_async_select_winner(
    first_kind: i64,
    first_handle: i64,
    second_kind: i64,
    second_handle: i64,
) -> i64 {
    unsafe { wait_for_first_ready_winner(first_kind, first_handle, second_kind, second_handle) }
}

#[cfg(feature = "native-bridge")]
define_async_select!(sengoo_async_select_i8, sengoo_async_result_dispatch_i8, i8);

#[cfg(feature = "native-bridge")]
define_async_select!(
    sengoo_async_select_i16,
    sengoo_async_result_dispatch_i16,
    i16
);

#[cfg(feature = "native-bridge")]
define_async_select!(
    sengoo_async_select_i32,
    sengoo_async_result_dispatch_i32,
    i32
);

#[cfg(feature = "native-bridge")]
define_async_select!(
    sengoo_async_select_i64,
    sengoo_async_result_dispatch_i64,
    i64
);

#[cfg(feature = "native-bridge")]
define_async_select!(
    sengoo_async_select_bool,
    sengoo_async_result_dispatch_bool,
    bool
);

#[cfg(feature = "native-bridge")]
define_async_select!(
    sengoo_async_select_f32,
    sengoo_async_result_dispatch_f32,
    f32
);

#[cfg(feature = "native-bridge")]
define_async_select!(
    sengoo_async_select_f64,
    sengoo_async_result_dispatch_f64,
    f64
);

#[cfg(feature = "native-bridge")]
#[no_mangle]
pub extern "C" fn sengoo_async_run_main_i64() -> i64 {
    let result = Arc::new(Mutex::new(None));
    let mut scheduler = CoroutineScheduler::new();
    scheduler.spawn(RootAsyncMainI64Task::new(result.clone()));

    CURRENT_SCHEDULER.with(|cell| {
        let previous = cell.replace(&mut scheduler);
        while !scheduler.is_empty() {
            let _ = scheduler.run_until_idle(1);
        }
        cell.set(previous);
    });

    let final_result = result
        .lock()
        .expect("async main result mutex poisoned")
        .unwrap_or_default();
    final_result
}

#[cfg(all(test, feature = "native-bridge"))]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicI64, AtomicU32, AtomicU8, Ordering};

    unsafe extern "C" {
        fn sengoo_async_run_main_i64() -> i64;
    }

    static TEST_POLLS: AtomicU32 = AtomicU32::new(0);
    static MAIN_MODE: AtomicU8 = AtomicU8::new(0);
    static MAIN_RESULT: AtomicI64 = AtomicI64::new(42);
    static MAIN_DEADLINE_OFFSET_MS: AtomicI64 = AtomicI64::new(0);
    static MAIN_DEADLINE: Mutex<Option<Instant>> = Mutex::new(None);
    static SELECT_HINT_POLLS: AtomicU32 = AtomicU32::new(0);
    static CANCEL_DISPATCH_CALLS: AtomicU32 = AtomicU32::new(0);
    static DROP_DISPATCH_CALLS: AtomicU32 = AtomicU32::new(0);
    static TEST_GUARD: Mutex<()> = Mutex::new(());

    #[test]
    fn async_handle_helpers_reject_zero_handles() {
        assert!(unsafe { handle_nonnull::<SleepFutureState>(0) }.is_none());
        assert!(unsafe { handle_ref::<SleepFutureState>(0) }.is_none());
        assert!(unsafe { handle_mut::<SleepFutureState>(0) }.is_none());
        assert!(unsafe { handle_take_box::<SleepFutureState>(0) }.is_none());
    }

    #[test]
    fn async_handle_helpers_round_trip_allocated_state() {
        let initial_deadline = Instant::now() + Duration::from_millis(3);
        let updated_deadline = initial_deadline + Duration::from_millis(2);
        let handle = Box::into_raw(Box::new(SleepFutureState {
            deadline: initial_deadline,
        })) as i64;

        let state = unsafe { handle_ref::<SleepFutureState>(handle) }
            .expect("allocated handle should decode to shared ref");
        assert_eq!(state.deadline, initial_deadline);

        unsafe { handle_mut::<SleepFutureState>(handle) }
            .expect("allocated handle should decode to mutable ref")
            .deadline = updated_deadline;

        let state = unsafe { handle_take_box::<SleepFutureState>(handle) }
            .expect("allocated handle should decode back into box");
        assert_eq!(state.deadline, updated_deadline);
    }

    #[test]
    fn merge_wakeup_hint_with_deadline_prefers_earlier_instant() {
        let now = Instant::now();
        let child_hint = now + Duration::from_millis(20);
        let deadline = now + Duration::from_millis(5);

        let merged = merge_wakeup_hint_with_deadline(Some(child_hint), deadline);
        assert_eq!(merged, deadline);

        let merged_without_child = merge_wakeup_hint_with_deadline(None, deadline);
        assert_eq!(merged_without_child, deadline);
    }

    #[test]
    fn scheduler_ffi_helpers_reject_null_pointers() {
        assert_eq!(
            unsafe { sengoo_async_scheduler_run_until_idle(std::ptr::null_mut(), 1) },
            0
        );
        assert_eq!(sengoo_async_spawn_raw(123, 456), 0);
        unsafe { sengoo_async_scheduler_free(std::ptr::null_mut()) };
    }

    #[no_mangle]
    pub extern "C" fn main__start() -> i64 {
        7
    }

    #[no_mangle]
    pub extern "C" fn main__poll(handle: i64) -> i64 {
        assert_eq!(handle, 7);
        let polls = TEST_POLLS.fetch_add(1, Ordering::SeqCst);
        match MAIN_MODE.load(Ordering::SeqCst) {
            0 => {
                if polls >= 2 {
                    1
                } else {
                    0
                }
            }
            1 => {
                let deadline = main_deadline();
                if polls == 0 {
                    record_poll_wakeup_hint(deadline);
                    0
                } else {
                    1
                }
            }
            2 => {
                let deadline = main_deadline();
                if polls == 0 {
                    record_poll_wakeup_hint(deadline);
                    0
                } else if Instant::now() >= deadline {
                    1
                } else {
                    record_poll_wakeup_hint(deadline);
                    0
                }
            }
            mode => {
                eprintln!("unexpected main poll mode: {mode}");
                1
            }
        }
    }

    #[no_mangle]
    pub extern "C" fn main__result(handle: i64) -> i64 {
        assert_eq!(handle, 7);
        MAIN_RESULT.load(Ordering::SeqCst)
    }

    #[no_mangle]
    pub extern "C" fn sengoo_async_poll_dispatch(kind: i64, handle: i64) -> i64 {
        if kind == async_spawn_kind_id_for_tests() {
            unsafe { sengoo_async_sleep__poll(handle) }
        } else if kind == async_select_hint_kind_id_for_tests() {
            SELECT_HINT_POLLS.fetch_add(1, Ordering::SeqCst);
            unsafe { sengoo_async_sleep__poll(handle) }
        } else {
            1
        }
    }

    #[no_mangle]
    pub unsafe extern "C" fn sengoo_async_cancel_dispatch(kind: i64, handle: i64) -> bool {
        if kind == async_spawn_kind_id_for_tests() {
            CANCEL_DISPATCH_CALLS.fetch_add(1, Ordering::SeqCst);
            sengoo_async_sleep__result(handle);
            true
        } else {
            false
        }
    }

    #[no_mangle]
    pub unsafe extern "C" fn sengoo_async_drop_dispatch(kind: i64, handle: i64) {
        if kind == async_spawn_kind_id_for_tests() {
            DROP_DISPATCH_CALLS.fetch_add(1, Ordering::SeqCst);
            sengoo_async_sleep__result(handle);
        }
    }

    macro_rules! define_test_result_dispatch {
        ($name:ident, $ty:ty, $value:expr) => {
            #[no_mangle]
            pub extern "C" fn $name(_kind: i64, _handle: i64) -> $ty {
                $value
            }
        };
    }

    define_test_result_dispatch!(sengoo_async_result_dispatch_i8, i8, 7);
    define_test_result_dispatch!(sengoo_async_result_dispatch_i16, i16, 7);
    define_test_result_dispatch!(sengoo_async_result_dispatch_i32, i32, 7);

    #[no_mangle]
    pub extern "C" fn sengoo_async_result_dispatch_i64(kind: i64, _handle: i64) -> i64 {
        if kind == async_select_hint_kind_id_for_tests() {
            11
        } else {
            42
        }
    }

    #[no_mangle]
    pub extern "C" fn sengoo_async_result_dispatch_bool(kind: i64, _handle: i64) -> bool {
        kind == async_select_hint_kind_id_for_tests()
    }

    define_test_result_dispatch!(sengoo_async_result_dispatch_f32, f32, 3.5);
    define_test_result_dispatch!(sengoo_async_result_dispatch_f64, f64, 3.5);

    struct CountDownTask(u8);

    impl CoroutineTask for CountDownTask {
        fn poll(&mut self) -> TaskState {
            if self.0 == 0 {
                TaskState::Complete
            } else {
                self.0 -= 1;
                TaskState::Pending
            }
        }
    }

    struct DeadlineHintTask {
        deadline: Instant,
        polls: Arc<AtomicU32>,
    }

    impl CoroutineTask for DeadlineHintTask {
        fn poll(&mut self) -> TaskState {
            self.polls.fetch_add(1, Ordering::SeqCst);
            if Instant::now() >= self.deadline {
                TaskState::Complete
            } else {
                record_poll_wakeup_hint(self.deadline);
                TaskState::Pending
            }
        }
    }

    #[test]
    fn scheduler_completes_tasks_over_multiple_ticks() {
        let _guard = TEST_GUARD.lock().expect("test guard mutex poisoned");
        let mut scheduler = CoroutineScheduler::new();
        let _a = scheduler.spawn(CountDownTask(0));
        let b = scheduler.spawn(CountDownTask(2));

        let first = scheduler.tick();
        assert_eq!(first.len(), 1);
        assert!(scheduler.stats().scheduled >= 1);

        let rest = scheduler.run_until_idle(8);
        assert!(rest.contains(&b));
        assert!(scheduler.is_empty());
        assert_eq!(scheduler.stats().completed, 2);
    }

    #[test]
    fn scheduler_cancel_marks_pending_task_as_canceled() {
        let _guard = TEST_GUARD.lock().expect("test guard mutex poisoned");
        let mut scheduler = CoroutineScheduler::new();
        let task = scheduler.spawn(CountDownTask(3));

        assert_eq!(scheduler.task_status(task) as i64, 1);
        assert!(scheduler.cancel(task));
        assert_eq!(scheduler.task_status(task) as i64, 3);
        assert!(scheduler.is_empty());
    }

    #[test]
    fn scheduler_task_status_tracks_completion() {
        let _guard = TEST_GUARD.lock().expect("test guard mutex poisoned");
        let mut scheduler = CoroutineScheduler::new();
        let task = scheduler.spawn(CountDownTask(0));

        assert_eq!(scheduler.task_status(task) as i64, 1);
        let finished = scheduler.tick();
        assert_eq!(finished, vec![task]);
        assert_eq!(scheduler.task_status(task) as i64, 2);
    }

    #[test]
    fn scheduler_ffi_cancel_and_status_handle_null_and_unknown_tasks() {
        let _guard = TEST_GUARD.lock().expect("test guard mutex poisoned");
        let scheduler = sengoo_async_scheduler_new();
        let task = unsafe { scheduler_mut(scheduler).expect("scheduler should be valid") }
            .spawn(CountDownTask(3));
        let task = task as i64;

        assert_eq!(
            unsafe { sengoo_async_scheduler_task_status(std::ptr::null_mut(), task) },
            0
        );
        assert!(!unsafe { sengoo_async_scheduler_cancel(std::ptr::null_mut(), task) });
        assert_eq!(
            unsafe { sengoo_async_scheduler_task_status(scheduler, task) },
            1
        );
        assert!(unsafe { sengoo_async_scheduler_cancel(scheduler, task) });
        assert_eq!(
            unsafe { sengoo_async_scheduler_task_status(scheduler, task) },
            3
        );
        assert_eq!(
            unsafe { sengoo_async_scheduler_task_status(scheduler, task + 999) },
            0
        );

        unsafe { sengoo_async_scheduler_free(scheduler) };
    }

    #[test]
    fn current_scheduler_task_wrappers_use_thread_local_scheduler() {
        let _guard = TEST_GUARD.lock().expect("test guard mutex poisoned");
        let mut scheduler = CoroutineScheduler::new();
        let task = scheduler.spawn(CountDownTask(3)) as i64;

        CURRENT_SCHEDULER.with(|cell| {
            let previous = cell.replace(&mut scheduler);
            assert_eq!(sengoo_async_task_status(task), 1);
            assert!(sengoo_async_cancel_task(task));
            assert_eq!(sengoo_async_task_status(task), 3);
            cell.set(previous);
        });
    }

    #[test]
    fn current_scheduler_task_wrappers_return_defaults_without_scheduler() {
        let _guard = TEST_GUARD.lock().expect("test guard mutex poisoned");
        CURRENT_SCHEDULER.with(|cell| {
            let previous = cell.replace(std::ptr::null_mut());
            assert_eq!(sengoo_async_task_status(1), 0);
            assert!(!sengoo_async_cancel_task(1));
            cell.set(previous);
        });
    }

    #[test]
    fn scheduler_ffi_cancel_uses_dispatch_for_foreign_tasks() {
        let _guard = TEST_GUARD.lock().expect("test guard mutex poisoned");
        CANCEL_DISPATCH_CALLS.store(0, Ordering::SeqCst);
        let scheduler = sengoo_async_scheduler_new();
        let foreign_task = unsafe { scheduler_mut(scheduler).expect("scheduler should be valid") }
            .spawn(ForeignAsyncTask {
                kind: async_spawn_kind_id_for_tests(),
                handle: sengoo_async_sleep__start(25),
            });
        let foreign_task = foreign_task as i64;

        assert_eq!(
            unsafe { sengoo_async_scheduler_task_status(scheduler, foreign_task) },
            1
        );
        assert!(unsafe { sengoo_async_scheduler_cancel(scheduler, foreign_task) });
        assert_eq!(
            unsafe { sengoo_async_scheduler_task_status(scheduler, foreign_task) },
            3
        );
        assert_eq!(CANCEL_DISPATCH_CALLS.load(Ordering::SeqCst), 1);

        unsafe { sengoo_async_scheduler_free(scheduler) };
    }

    #[test]
    fn scheduler_free_drops_pending_foreign_tasks_via_dispatch() {
        let _guard = TEST_GUARD.lock().expect("test guard mutex poisoned");
        DROP_DISPATCH_CALLS.store(0, Ordering::SeqCst);
        let scheduler = sengoo_async_scheduler_new();
        let _foreign_task = unsafe { scheduler_mut(scheduler).expect("scheduler should be valid") }
            .spawn(ForeignAsyncTask {
                kind: async_spawn_kind_id_for_tests(),
                handle: sengoo_async_sleep__start(25),
            });

        unsafe { sengoo_async_scheduler_free(scheduler) };
        assert_eq!(DROP_DISPATCH_CALLS.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn sleep_helper_completes_after_deadline() {
        let _guard = TEST_GUARD.lock().expect("test guard mutex poisoned");
        let handle = sengoo_async_sleep__start(5);
        assert_eq!(unsafe { sengoo_async_sleep__poll(handle) }, 0);
        std::thread::sleep(Duration::from_millis(8));
        assert_eq!(unsafe { sengoo_async_sleep__poll(handle) }, 1);
        unsafe { sengoo_async_sleep__result(handle) };
    }

    #[test]
    fn timeout_bool_future_can_complete_with_false_after_deadline() {
        let _guard = TEST_GUARD.lock().expect("test guard mutex poisoned");
        let child_handle = sengoo_async_sleep__start(20);
        let timeout_handle =
            sengoo_async_timeout_bool__start(async_spawn_kind_id_for_tests(), child_handle, 1);
        assert_eq!(
            unsafe { sengoo_async_timeout_bool__poll(timeout_handle) },
            0
        );
        std::thread::sleep(Duration::from_millis(5));
        assert_eq!(
            unsafe { sengoo_async_timeout_bool__poll(timeout_handle) },
            1
        );
        assert!(!unsafe { sengoo_async_timeout_bool__result(timeout_handle) });
        unsafe { sengoo_async_sleep__result(child_handle) };
    }

    #[test]
    fn select_i64_waits_for_deadline_hints_instead_of_busy_spinning() {
        let _guard = TEST_GUARD.lock().expect("test guard mutex poisoned");
        let fast = sengoo_async_sleep__start(12);
        let slow = sengoo_async_sleep__start(20);
        SELECT_HINT_POLLS.store(0, Ordering::SeqCst);

        let start = Instant::now();
        let result = sengoo_async_select_i64(
            async_select_hint_kind_id_for_tests(),
            fast,
            async_select_hint_kind_id_for_tests(),
            slow,
        );
        let elapsed = start.elapsed();

        assert_eq!(result, 11);
        assert!(elapsed >= Duration::from_millis(10));
        assert!(
            SELECT_HINT_POLLS.load(Ordering::SeqCst) <= 4,
            "select should not busy-spin when wakeup hints are available"
        );
        unsafe { sengoo_async_sleep__result(slow) };
    }

    #[test]
    fn select_bool_returns_dispatched_bool_value() {
        let _guard = TEST_GUARD.lock().expect("test guard mutex poisoned");
        let ready = sengoo_async_sleep__start(0);
        let pending = sengoo_async_sleep__start(10);

        let result = sengoo_async_select_bool(
            async_select_hint_kind_id_for_tests(),
            ready,
            async_spawn_kind_id_for_tests(),
            pending,
        );

        assert!(result);
        unsafe { sengoo_async_sleep__result(pending) };
    }

    #[test]
    fn select_winner_returns_zero_when_first_future_wins() {
        let _guard = TEST_GUARD.lock().expect("test guard mutex poisoned");
        let ready = sengoo_async_sleep__start(0);
        let pending = sengoo_async_sleep__start(10);

        let winner = sengoo_async_select_winner(
            async_select_hint_kind_id_for_tests(),
            ready,
            async_spawn_kind_id_for_tests(),
            pending,
        );

        assert_eq!(winner, 0);
        unsafe {
            sengoo_async_sleep__result(ready);
            sengoo_async_sleep__result(pending);
        }
    }

    #[test]
    fn select_winner_returns_one_when_second_future_wins() {
        let _guard = TEST_GUARD.lock().expect("test guard mutex poisoned");
        let pending = sengoo_async_sleep__start(10);
        let ready = sengoo_async_sleep__start(0);

        let winner = sengoo_async_select_winner(
            async_spawn_kind_id_for_tests(),
            pending,
            async_select_hint_kind_id_for_tests(),
            ready,
        );

        assert_eq!(winner, 1);
        unsafe {
            sengoo_async_sleep__result(pending);
            sengoo_async_sleep__result(ready);
        }
    }

    #[test]
    fn scheduler_waits_for_deadline_hints_instead_of_busy_spinning() {
        let _guard = TEST_GUARD.lock().expect("test guard mutex poisoned");
        let mut scheduler = CoroutineScheduler::new();
        let polls = Arc::new(AtomicU32::new(0));
        scheduler.spawn(DeadlineHintTask {
            deadline: Instant::now() + Duration::from_millis(12),
            polls: polls.clone(),
        });

        let start = Instant::now();
        let finished = scheduler.run_until_idle(2);
        let elapsed = start.elapsed();

        assert_eq!(finished.len(), 1);
        assert!(scheduler.is_empty());
        assert!(elapsed >= Duration::from_millis(10));
        assert!(polls.load(Ordering::SeqCst) <= 2);
    }

    #[test]
    fn ffi_bridge_runs_root_i64_task_to_completion() {
        let _guard = TEST_GUARD.lock().expect("test guard mutex poisoned");
        MAIN_MODE.store(0, Ordering::SeqCst);
        MAIN_RESULT.store(42, Ordering::SeqCst);
        *MAIN_DEADLINE.lock().expect("main deadline mutex poisoned") = None;
        TEST_POLLS.store(0, Ordering::SeqCst);
        let result = unsafe { sengoo_async_run_main_i64() };
        assert_eq!(result, 42);
        assert_eq!(TEST_POLLS.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn ffi_bridge_root_task_respects_deadline_hints() {
        let _guard = TEST_GUARD.lock().expect("test guard mutex poisoned");
        MAIN_MODE.store(2, Ordering::SeqCst);
        MAIN_RESULT.store(99, Ordering::SeqCst);
        MAIN_DEADLINE_OFFSET_MS.store(12, Ordering::SeqCst);
        *MAIN_DEADLINE.lock().expect("main deadline mutex poisoned") = Some(
            Instant::now()
                + Duration::from_millis(MAIN_DEADLINE_OFFSET_MS.load(Ordering::SeqCst) as u64),
        );
        TEST_POLLS.store(0, Ordering::SeqCst);

        let start = Instant::now();
        let result = unsafe { sengoo_async_run_main_i64() };
        let elapsed = start.elapsed();

        assert_eq!(result, 99);
        assert!(elapsed >= Duration::from_millis(10));
        assert!(TEST_POLLS.load(Ordering::SeqCst) <= 2);
    }

    #[test]
    fn ffi_bridge_unknown_main_poll_mode_falls_back_to_ready() {
        let _guard = TEST_GUARD.lock().expect("test guard mutex poisoned");
        MAIN_MODE.store(99, Ordering::SeqCst);
        MAIN_RESULT.store(21, Ordering::SeqCst);
        *MAIN_DEADLINE.lock().expect("main deadline mutex poisoned") = None;
        TEST_POLLS.store(0, Ordering::SeqCst);

        let result = unsafe { sengoo_async_run_main_i64() };

        assert_eq!(result, 21);
        assert_eq!(TEST_POLLS.load(Ordering::SeqCst), 1);
    }

    fn main_deadline() -> Instant {
        MAIN_DEADLINE
            .lock()
            .expect("main deadline mutex poisoned")
            .expect("main deadline must be configured for this mode")
    }

    fn async_spawn_kind_id_for_tests() -> i64 {
        1
    }

    fn async_select_hint_kind_id_for_tests() -> i64 {
        99
    }
}
