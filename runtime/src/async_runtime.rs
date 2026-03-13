//! Coroutine-compatible async runtime scheduling primitives.

use std::collections::VecDeque;
use std::time::Instant;
#[cfg(feature = "native-bridge")]
use std::cell::Cell;
#[cfg(feature = "native-bridge")]
use std::sync::{Arc, Mutex};
#[cfg(feature = "native-bridge")]
use std::time::Duration;

pub type TaskId = u64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskState {
    Pending,
    Complete,
}

pub trait CoroutineTask {
    fn poll(&mut self) -> TaskState;
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
}

impl Default for CoroutineScheduler {
    fn default() -> Self {
        Self::new()
    }
}

impl CoroutineScheduler {
    pub fn new() -> Self {
        Self {
            next_id: 1,
            queue: VecDeque::new(),
            completed: 0,
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

            if entry
                .next_wakeup
                .is_some_and(|deadline| deadline > now)
            {
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
    fn sengoo_async_result_dispatch_i64(kind: i64, handle: i64) -> i64;
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
struct RootAsyncMainI64Task {
    handle: Option<i64>,
    result: Arc<Mutex<Option<i64>>>,
}

#[cfg(feature = "native-bridge")]
impl RootAsyncMainI64Task {
    fn new(result: Arc<Mutex<Option<i64>>>) -> Self {
        Self { handle: None, result }
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
        *self.result.lock().expect("async main result mutex poisoned") = Some(result);
        TaskState::Complete
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
    if handle == 0 {
        return 1;
    }
    let state = &*(handle as *const SleepFutureState);
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
    if handle == 0 {
        return;
    }
    drop(Box::from_raw(handle as *mut SleepFutureState));
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
    if handle == 0 {
        return 1;
    }

    let state = &mut *(handle as *mut TimeoutBoolFutureState);
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

    record_poll_wakeup_hint(child_hint.map_or(state.deadline, |hint| hint.min(state.deadline)));
    0
}

#[cfg(feature = "native-bridge")]
#[no_mangle]
pub unsafe extern "C" fn sengoo_async_timeout_bool__result(handle: i64) -> bool {
    if handle == 0 {
        return false;
    }

    let state = Box::from_raw(handle as *mut TimeoutBoolFutureState);
    state.result.unwrap_or(false)
}

#[cfg(feature = "native-bridge")]
#[no_mangle]
pub extern "C" fn sengoo_async_scheduler_new() -> *mut CoroutineScheduler {
    Box::into_raw(Box::new(CoroutineScheduler::new()))
}

#[cfg(feature = "native-bridge")]
#[no_mangle]
pub unsafe extern "C" fn sengoo_async_scheduler_free(
    scheduler: *mut CoroutineScheduler,
) {
    if scheduler.is_null() {
        return;
    }
    drop(Box::from_raw(scheduler));
}

#[cfg(feature = "native-bridge")]
#[no_mangle]
pub unsafe extern "C" fn sengoo_async_scheduler_run_until_idle(
    scheduler: *mut CoroutineScheduler,
    max_ticks: usize,
) -> usize {
    if scheduler.is_null() {
        return 0;
    }
    CURRENT_SCHEDULER.with(|cell| {
        let previous = cell.replace(scheduler);
        let finished = (&mut *scheduler).run_until_idle(max_ticks).len();
        cell.set(previous);
        finished
    })
}

#[cfg(feature = "native-bridge")]
#[no_mangle]
pub extern "C" fn sengoo_async_spawn_raw(kind: i64, handle: i64) -> i64 {
    CURRENT_SCHEDULER.with(|cell| {
        let scheduler = cell.get();
        if scheduler.is_null() {
            return 0;
        }
        unsafe { (&mut *scheduler).spawn(ForeignAsyncTask { kind, handle }) as i64 }
    })
}

#[cfg(feature = "native-bridge")]
#[no_mangle]
pub extern "C" fn sengoo_async_select_i64(
    first_kind: i64,
    first_handle: i64,
    second_kind: i64,
    second_handle: i64,
) -> i64 {
    loop {
        clear_poll_wakeup_hint();
        if unsafe { sengoo_async_poll_dispatch(first_kind, first_handle) } != 0 {
            return unsafe { sengoo_async_result_dispatch_i64(first_kind, first_handle) };
        }
        let first_hint = take_poll_wakeup_hint();

        clear_poll_wakeup_hint();
        if unsafe { sengoo_async_poll_dispatch(second_kind, second_handle) } != 0 {
            return unsafe { sengoo_async_result_dispatch_i64(second_kind, second_handle) };
        }
        let second_hint = take_poll_wakeup_hint();

        wait_for_wakeup_hint_or_yield(merge_wakeup_hints(first_hint, second_hint));
    }
}

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
    static TEST_GUARD: Mutex<()> = Mutex::new(());

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
                if polls >= 2 { 1 } else { 0 }
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
            mode => panic!("unexpected main poll mode: {mode}"),
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
    pub extern "C" fn sengoo_async_result_dispatch_i64(kind: i64, _handle: i64) -> i64 {
        if kind == async_select_hint_kind_id_for_tests() {
            11
        } else {
            42
        }
    }

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
        assert_eq!(unsafe { sengoo_async_timeout_bool__poll(timeout_handle) }, 0);
        std::thread::sleep(Duration::from_millis(5));
        assert_eq!(unsafe { sengoo_async_timeout_bool__poll(timeout_handle) }, 1);
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

    fn main_deadline() -> Instant {
        MAIN_DEADLINE
            .lock()
            .expect("main deadline mutex poisoned")
            .expect("main deadline must be configured for this mode")
    }

    fn async_spawn_kind_id_for_tests() -> i64 {
        hash_kind_id_for_tests(b"sengoo_async_sleep")
    }

    fn async_select_hint_kind_id_for_tests() -> i64 {
        hash_kind_id_for_tests(b"sengoo_async_select_hint")
    }

    fn hash_kind_id_for_tests(name: &[u8]) -> i64 {
        let mut hash = 0x811c9dc5u32;
        for byte in name {
            hash ^= u32::from(*byte);
            hash = hash.wrapping_mul(0x01000193);
        }
        i64::from(hash)
    }
}
