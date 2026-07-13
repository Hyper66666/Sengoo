//! Coroutine-compatible async runtime scheduling primitives.

#[cfg(feature = "native-bridge")]
use std::cell::Cell;
use std::collections::{HashMap, VecDeque};
use std::ptr::NonNull;
#[cfg(all(test, feature = "native-bridge"))]
use std::sync::{Arc, Mutex};
#[cfg(all(test, feature = "native-bridge"))]
use std::time::Duration;
use std::time::Instant;

#[cfg(feature = "native-bridge")]
mod bridge;
#[cfg(feature = "native-bridge")]
mod concurrent;
#[cfg(feature = "native-bridge")]
mod executor;
#[cfg(feature = "native-bridge")]
mod futures;
mod reactor;
#[cfg(feature = "native-bridge")]
mod select;
#[cfg(feature = "native-bridge")]
mod task_scope;
#[cfg(feature = "native-bridge")]
mod thread_pool;

#[cfg(all(test, feature = "native-bridge"))]
use bridge::{scheduler_mut, ForeignAsyncTask, CURRENT_SCHEDULER};
#[cfg(feature = "native-bridge")]
pub use bridge::{
    sengoo_async_cancel_task, sengoo_async_join_task, sengoo_async_run_main_i64,
    sengoo_async_runtime_enable_executor, sengoo_async_runtime_executor_enabled,
    sengoo_async_runtime_shutdown_executor, sengoo_async_scheduler_cancel,
    sengoo_async_scheduler_free, sengoo_async_scheduler_new, sengoo_async_scheduler_run_until_idle,
    sengoo_async_scheduler_task_status, sengoo_async_spawn_raw, sengoo_async_spawn_task_raw,
    sengoo_async_task_status,
};
#[cfg(feature = "native-bridge")]
pub use concurrent::{
    sengoo_arc_borrow_ptr, sengoo_arc_clone, sengoo_arc_drop, sengoo_arc_new, sengoo_arc_new_parts,
    sengoo_arc_strong_count, sengoo_async_channel_bounded_i64, sengoo_async_channel_pair_free,
    sengoo_async_channel_pair_receiver, sengoo_async_channel_pair_sender,
    sengoo_async_channel_recv_i64__cancel, sengoo_async_channel_recv_i64__drop,
    sengoo_async_channel_recv_i64__poll, sengoo_async_channel_recv_i64__result,
    sengoo_async_channel_recv_i64__start, sengoo_async_channel_send_i64__cancel,
    sengoo_async_channel_send_i64__drop, sengoo_async_channel_send_i64__poll,
    sengoo_async_channel_send_i64__result, sengoo_async_channel_send_i64__start,
    sengoo_async_channel_sender_clone, sengoo_async_channel_sender_close,
    sengoo_async_channel_sender_drop, sengoo_async_mutex_close, sengoo_async_mutex_drop,
    sengoo_async_mutex_guard_copy_into, sengoo_async_mutex_guard_get,
    sengoo_async_mutex_guard_get_i64, sengoo_async_mutex_guard_set,
    sengoo_async_mutex_guard_set_i64, sengoo_async_mutex_guard_unlock,
    sengoo_async_mutex_guard_unlock_i64, sengoo_async_mutex_lock__cancel,
    sengoo_async_mutex_lock__drop, sengoo_async_mutex_lock__poll, sengoo_async_mutex_lock__result,
    sengoo_async_mutex_lock__start, sengoo_async_mutex_lock_i64__cancel,
    sengoo_async_mutex_lock_i64__drop, sengoo_async_mutex_lock_i64__poll,
    sengoo_async_mutex_lock_i64__result, sengoo_async_mutex_lock_i64__start,
    sengoo_async_mutex_new, sengoo_async_mutex_new_i64, sengoo_async_mutex_new_parts,
    sengoo_async_mutex_unlock_i64, sengoo_async_runtime_enable_thread_pool,
    sengoo_async_runtime_thread_pool_enabled, sengoo_async_rwlock_close, sengoo_async_rwlock_drop,
    sengoo_async_rwlock_new, sengoo_async_rwlock_new_i64, sengoo_async_rwlock_new_parts,
    sengoo_async_rwlock_read_guard_copy_into, sengoo_async_rwlock_read_guard_get_i64,
    sengoo_async_rwlock_read_guard_unlock, sengoo_async_rwlock_read_guard_unlock_i64,
    sengoo_async_rwlock_try_read, sengoo_async_rwlock_try_read_i64, sengoo_async_rwlock_try_write,
    sengoo_async_rwlock_try_write_i64, sengoo_async_rwlock_write_guard_copy_into,
    sengoo_async_rwlock_write_guard_get_i64, sengoo_async_rwlock_write_guard_set,
    sengoo_async_rwlock_write_guard_set_i64, sengoo_async_rwlock_write_guard_unlock,
    sengoo_async_rwlock_write_guard_unlock_i64, sengoo_async_shared_counter_clone_i64,
    sengoo_async_shared_counter_drop, sengoo_async_shared_counter_get_i64,
    sengoo_async_shared_counter_job_drop, sengoo_async_shared_counter_join_i64,
    sengoo_async_shared_counter_new_i64, sengoo_async_shared_counter_spawn_add_i64,
    sengoo_async_spawn_blocking_i64__cancel, sengoo_async_spawn_blocking_i64__drop,
    sengoo_async_spawn_blocking_i64__poll, sengoo_async_spawn_blocking_i64__result,
    sengoo_async_spawn_blocking_i64__start, ChannelRecvI64Result, ChannelSendI64Result,
    MutexLockI64Result,
};
#[cfg(feature = "native-bridge")]
pub use futures::{
    sengoo_async_sleep__cancel, sengoo_async_sleep__drop, sengoo_async_sleep__poll,
    sengoo_async_sleep__result, sengoo_async_sleep__start, sengoo_async_timeout_bool__cancel,
    sengoo_async_timeout_bool__drop, sengoo_async_timeout_bool__poll,
    sengoo_async_timeout_bool__result, sengoo_async_timeout_bool__start,
    sengoo_async_timeout_cancel_i64__cancel, sengoo_async_timeout_cancel_i64__drop,
    sengoo_async_timeout_cancel_i64__poll, sengoo_async_timeout_cancel_i64__result,
    sengoo_async_timeout_cancel_i64__start, TimeoutCancelI64Result,
};
#[cfg(all(test, feature = "native-bridge"))]
use futures::{PollLifecycle, SleepFutureState, POLL_ERROR_COMPLETED, POLL_ERROR_REENTRANT};
#[cfg(test)]
pub(crate) use reactor::http_listener_interest_count;
pub(crate) use reactor::{
    http_listener_poll_accept, http_listener_register, http_listener_unregister,
};
pub use reactor::{
    sengoo_async_reactor_fd_readable_register, sengoo_async_reactor_tcp_readable_register,
    sengoo_async_reactor_timer_register, sengoo_async_reactor_unregister,
    sengoo_async_reactor_wait__cancel, sengoo_async_reactor_wait__drop,
    sengoo_async_reactor_wait__poll, sengoo_async_reactor_wait__result,
    sengoo_async_reactor_wait__start,
};
#[cfg(feature = "native-bridge")]
pub use select::{
    sengoo_async_select_bool, sengoo_async_select_cancel_n_winner,
    sengoo_async_select_cancel_winner, sengoo_async_select_f32, sengoo_async_select_f64,
    sengoo_async_select_i16, sengoo_async_select_i32, sengoo_async_select_i64,
    sengoo_async_select_i8, sengoo_async_select_n_winner, sengoo_async_select_winner,
};
#[cfg(feature = "native-bridge")]
pub use task_scope::{
    sengoo_async_task_scope_cancel_join, sengoo_async_task_scope_join, sengoo_async_task_scope_new,
    sengoo_async_task_scope_spawn_raw,
};

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
    Failed = 4,
}

pub trait CoroutineTask {
    fn poll(&mut self) -> TaskState;

    fn foreign_identity(&self) -> Option<(i64, i64)> {
        None
    }

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

    pub fn cancel_foreign(&mut self, kind: i64, handle: i64) -> bool {
        let Some(index) = self
            .queue
            .iter()
            .position(|entry| entry.task.foreign_identity() == Some((kind, handle)))
        else {
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

        let task_id = self
            .queue
            .remove(index)
            .expect("foreign task index located in queue should remain valid")
            .id;
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
        #[cfg(feature = "native-bridge")]
        if thread_pool::take_cross_thread_wakeup() {
            return;
        }

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
            let sleep_for = deadline.saturating_duration_since(now);
            #[cfg(feature = "native-bridge")]
            thread_pool::wait_for_cross_thread_wakeup(sleep_for);
            #[cfg(not(feature = "native-bridge"))]
            std::thread::sleep(sleep_for);
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

#[cfg(feature = "native-bridge")]
pub(crate) fn record_external_poll_wakeup_hint(deadline: Instant) {
    record_poll_wakeup_hint(deadline);
}

#[cfg(not(feature = "native-bridge"))]
#[allow(dead_code)]
fn record_poll_wakeup_hint(_deadline: Instant) {}

#[cfg(not(feature = "native-bridge"))]
pub(crate) fn record_external_poll_wakeup_hint(_deadline: Instant) {}

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

unsafe fn handle_mut<'a, T>(handle: i64) -> Option<&'a mut T> {
    handle_nonnull(handle).map(|mut ptr| ptr.as_mut())
}

unsafe fn handle_take_box<T>(handle: i64) -> Option<Box<T>> {
    handle_nonnull(handle).map(|ptr| Box::from_raw(ptr.as_ptr()))
}

#[cfg(all(test, feature = "native-bridge"))]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU32, AtomicU8, Ordering};

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
            lifecycle: PollLifecycle::default(),
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
    fn poll_lifecycle_rejects_reentrant_and_completed_polls() {
        let lifecycle = PollLifecycle::default();
        let active = lifecycle.enter().expect("first poll should enter");
        assert_eq!(lifecycle.enter().err(), Some(POLL_ERROR_REENTRANT));
        active.mark_ready();
        assert_eq!(lifecycle.enter().err(), Some(POLL_ERROR_COMPLETED));
    }

    #[test]
    fn sleep_future_poll_after_ready_returns_stable_error() {
        let handle = sengoo_async_sleep__start(0);
        assert_eq!(unsafe { sengoo_async_sleep__poll(handle) }, 1);
        assert_eq!(
            unsafe { sengoo_async_sleep__poll(handle) },
            POLL_ERROR_COMPLETED
        );
        unsafe { sengoo_async_sleep__result(handle) };
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
    fn scheduler_cancel_does_not_demote_completed_tasks() {
        let _guard = TEST_GUARD.lock().expect("test guard mutex poisoned");
        let mut scheduler = CoroutineScheduler::new();
        let task = scheduler.spawn(CountDownTask(0));

        assert_eq!(scheduler.tick(), vec![task]);
        assert_eq!(scheduler.task_status(task) as i64, 2);
        assert!(!scheduler.cancel(task));
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
    fn current_scheduler_join_drives_cooperative_task_to_completion() {
        let _guard = TEST_GUARD.lock().expect("test guard mutex poisoned");
        let mut scheduler = CoroutineScheduler::new();
        let task = scheduler.spawn(CountDownTask(3)) as i64;

        CURRENT_SCHEDULER.with(|cell| {
            let previous = cell.replace(&mut scheduler);
            assert_eq!(
                sengoo_async_join_task(task),
                TaskLifecycleStatus::Completed as i64
            );
            assert_eq!(sengoo_async_task_status(task), 2);
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
    fn current_scheduler_can_cancel_foreign_task_by_kind_and_handle() {
        let _guard = TEST_GUARD.lock().expect("test guard mutex poisoned");
        CANCEL_DISPATCH_CALLS.store(0, Ordering::SeqCst);
        let mut scheduler = CoroutineScheduler::new();
        let handle = sengoo_async_sleep__start(25);
        let task = scheduler.spawn(ForeignAsyncTask {
            kind: async_spawn_kind_id_for_tests(),
            handle,
        });

        CURRENT_SCHEDULER.with(|cell| {
            let previous = cell.replace(&mut scheduler);
            assert!(bridge::cancel_scheduled_foreign(
                async_spawn_kind_id_for_tests(),
                handle
            ));
            cell.set(previous);
        });

        assert_eq!(scheduler.task_status(task) as i64, 3);
        assert_eq!(scheduler.stats().scheduled, 0);
        assert_eq!(CANCEL_DISPATCH_CALLS.load(Ordering::SeqCst), 1);
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
    fn select_n_winner_returns_first_ready_operand_with_rotation() {
        let _guard = TEST_GUARD.lock().expect("test guard mutex poisoned");
        let a = sengoo_async_sleep__start(15);
        let b = sengoo_async_sleep__start(0);
        let c = sengoo_async_sleep__start(20);

        let winner = sengoo_async_select_n_winner(
            3,
            async_spawn_kind_id_for_tests(),
            a,
            async_select_hint_kind_id_for_tests(),
            b,
            async_spawn_kind_id_for_tests(),
            c,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
        );

        assert_eq!(winner, 1);
        unsafe {
            sengoo_async_sleep__result(b);
            sengoo_async_sleep__result(a);
            sengoo_async_sleep__result(c);
        }
    }

    #[test]
    fn timeout_cancel_i64_returns_status_timeout_after_deadline() {
        let _guard = TEST_GUARD.lock().expect("test guard mutex poisoned");
        let child = sengoo_async_sleep__start(50);
        let handle =
            sengoo_async_timeout_cancel_i64__start(async_spawn_kind_id_for_tests(), child, 1);
        assert_eq!(unsafe { sengoo_async_timeout_cancel_i64__poll(handle) }, 0);
        std::thread::sleep(Duration::from_millis(5));
        assert_eq!(unsafe { sengoo_async_timeout_cancel_i64__poll(handle) }, 1);
        let result = unsafe { sengoo_async_timeout_cancel_i64__result(handle) };
        assert!(!result.is_ok);
        assert_eq!(result.error, 11);
    }

    #[test]
    fn reactor_timer_registration_unblocks_wait_future() {
        let _guard = TEST_GUARD.lock().expect("test guard mutex poisoned");
        let interest = sengoo_async_reactor_timer_register(1);
        let child = sengoo_async_sleep__start(50);
        let handle =
            sengoo_async_reactor_wait__start(interest, async_spawn_kind_id_for_tests(), child);
        assert_eq!(unsafe { sengoo_async_reactor_wait__poll(handle) }, 0);
        std::thread::sleep(Duration::from_millis(5));
        assert_eq!(unsafe { sengoo_async_reactor_wait__poll(handle) }, 1);
        unsafe {
            sengoo_async_reactor_wait__result(handle);
            sengoo_async_sleep__result(child);
        }
    }

    #[cfg(unix)]
    unsafe extern "C" {
        fn pipe(fds: *mut i32) -> i32;
        fn write(fd: i32, data: *const core::ffi::c_void, len: usize) -> isize;
        fn close(fd: i32) -> i32;
    }

    #[cfg(windows)]
    unsafe extern "C" {
        fn _pipe(fds: *mut i32, size: u32, mode: i32) -> i32;
        fn _write(fd: i32, data: *const core::ffi::c_void, len: u32) -> i32;
        fn _close(fd: i32) -> i32;
    }

    #[test]
    fn reactor_owned_fd_registration_observes_pipe_readiness() {
        let _guard = TEST_GUARD.lock().expect("test guard mutex poisoned");
        let mut fds = [-1i32; 2];
        #[cfg(unix)]
        assert_eq!(unsafe { pipe(fds.as_mut_ptr()) }, 0);
        #[cfg(windows)]
        assert_eq!(unsafe { _pipe(fds.as_mut_ptr(), 4096, 0x8000) }, 0);

        let interest = sengoo_async_reactor_fd_readable_register(i64::from(fds[0]));
        let child = sengoo_async_sleep__start(50);
        let handle =
            sengoo_async_reactor_wait__start(interest, async_spawn_kind_id_for_tests(), child);
        assert_eq!(unsafe { sengoo_async_reactor_wait__poll(handle) }, 0);

        let byte = [b'x'];
        #[cfg(unix)]
        assert_eq!(
            unsafe { write(fds[1], byte.as_ptr().cast(), byte.len()) },
            1
        );
        #[cfg(windows)]
        assert_eq!(unsafe { _write(fds[1], byte.as_ptr().cast(), 1) }, 1);

        assert_eq!(unsafe { sengoo_async_reactor_wait__poll(handle) }, 1);
        unsafe {
            sengoo_async_reactor_wait__result(handle);
            sengoo_async_sleep__result(child);
        }

        #[cfg(unix)]
        unsafe {
            close(fds[0]);
            close(fds[1]);
        }
        #[cfg(windows)]
        unsafe {
            _close(fds[0]);
            _close(fds[1]);
        }
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
    fn select_cancel_winner_cancels_loser_before_returning() {
        let _guard = TEST_GUARD.lock().expect("test guard mutex poisoned");
        CANCEL_DISPATCH_CALLS.store(0, Ordering::SeqCst);
        DROP_DISPATCH_CALLS.store(0, Ordering::SeqCst);
        let loser = sengoo_async_sleep__start(20);
        let winner = sengoo_async_sleep__start(0);

        let selected = sengoo_async_select_cancel_winner(
            async_spawn_kind_id_for_tests(),
            loser,
            async_select_hint_kind_id_for_tests(),
            winner,
        );

        assert_eq!(selected, 1);
        assert_eq!(CANCEL_DISPATCH_CALLS.load(Ordering::SeqCst), 1);
        assert_eq!(DROP_DISPATCH_CALLS.load(Ordering::SeqCst), 0);
        unsafe { sengoo_async_sleep__result(winner) };
    }

    #[test]
    fn select_cancel_duplicate_winner_handle_is_not_canceled_as_loser() {
        let _guard = TEST_GUARD.lock().expect("test guard mutex poisoned");
        CANCEL_DISPATCH_CALLS.store(0, Ordering::SeqCst);
        DROP_DISPATCH_CALLS.store(0, Ordering::SeqCst);
        let shared = sengoo_async_sleep__start(0);

        let selected = sengoo_async_select_cancel_winner(
            async_spawn_kind_id_for_tests(),
            shared,
            async_spawn_kind_id_for_tests(),
            shared,
        );

        assert_eq!(selected, 0);
        assert_eq!(CANCEL_DISPATCH_CALLS.load(Ordering::SeqCst), 0);
        assert_eq!(DROP_DISPATCH_CALLS.load(Ordering::SeqCst), 0);
        unsafe { sengoo_async_sleep__result(shared) };
    }

    #[test]
    fn select_cancel_n_winner_cancels_all_losers() {
        let _guard = TEST_GUARD.lock().expect("test guard mutex poisoned");
        CANCEL_DISPATCH_CALLS.store(0, Ordering::SeqCst);
        DROP_DISPATCH_CALLS.store(0, Ordering::SeqCst);
        let loser_a = sengoo_async_sleep__start(20);
        let winner = sengoo_async_sleep__start(0);
        let loser_b = sengoo_async_sleep__start(25);

        let selected = sengoo_async_select_cancel_n_winner(
            3,
            async_spawn_kind_id_for_tests(),
            loser_a,
            async_select_hint_kind_id_for_tests(),
            winner,
            async_spawn_kind_id_for_tests(),
            loser_b,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
        );

        assert_eq!(selected, 1);
        assert_eq!(CANCEL_DISPATCH_CALLS.load(Ordering::SeqCst), 2);
        assert_eq!(DROP_DISPATCH_CALLS.load(Ordering::SeqCst), 0);
        unsafe { sengoo_async_sleep__result(winner) };
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
        let mut finished = Vec::new();
        while !scheduler.is_empty() && start.elapsed() < Duration::from_millis(100) {
            finished.extend(scheduler.run_until_idle(1));
        }
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

    #[test]
    fn concurrent_thread_pool_rejects_invalid_worker_count() {
        let _guard = TEST_GUARD.lock().expect("test guard mutex poisoned");
        thread_pool::test_only_disable_thread_pool();
        assert_eq!(
            sengoo_async_runtime_enable_thread_pool(0),
            -thread_pool::STATUS_INVALID_ARGUMENT
        );
    }

    #[test]
    fn concurrent_detached_spawn_drops_completed_future_frame_exactly_once() {
        let _guard = TEST_GUARD.lock().expect("test guard mutex poisoned");
        let _executor_guard = executor::EXECUTOR_TEST_GUARD
            .lock()
            .expect("executor test guard mutex poisoned");
        executor::shutdown(executor::ConcurrentShutdownMode::Cancel);
        DROP_DISPATCH_CALLS.store(0, Ordering::SeqCst);
        assert_eq!(sengoo_async_runtime_enable_executor(2, 4), 1);

        let future = sengoo_async_sleep__start(0);
        let task = sengoo_async_spawn_task_raw(async_spawn_kind_id_for_tests(), future);
        assert!(task > 0);
        assert_eq!(
            sengoo_async_join_task(task),
            TaskLifecycleStatus::Completed as i64
        );
        assert_eq!(DROP_DISPATCH_CALLS.load(Ordering::SeqCst), 1);

        assert_eq!(sengoo_async_runtime_shutdown_executor(false), 0);
    }

    #[test]
    fn concurrent_rejected_detached_spawn_releases_future_frame_exactly_once() {
        let _guard = TEST_GUARD.lock().expect("test guard mutex poisoned");
        let _executor_guard = executor::EXECUTOR_TEST_GUARD
            .lock()
            .expect("executor test guard mutex poisoned");
        executor::shutdown(executor::ConcurrentShutdownMode::Cancel);
        CANCEL_DISPATCH_CALLS.store(0, Ordering::SeqCst);
        DROP_DISPATCH_CALLS.store(0, Ordering::SeqCst);
        assert_eq!(sengoo_async_runtime_enable_executor(1, 1), 1);

        let accepted = sengoo_async_spawn_task_raw(
            async_spawn_kind_id_for_tests(),
            sengoo_async_sleep__start(1_000),
        );
        assert!(accepted > 0);
        let rejected = sengoo_async_spawn_task_raw(
            async_spawn_kind_id_for_tests(),
            sengoo_async_sleep__start(1_000),
        );
        assert_eq!(rejected, 0);
        assert_eq!(CANCEL_DISPATCH_CALLS.load(Ordering::SeqCst), 1);
        assert_eq!(DROP_DISPATCH_CALLS.load(Ordering::SeqCst), 0);

        assert_eq!(sengoo_async_runtime_shutdown_executor(true), 0);
        assert_eq!(CANCEL_DISPATCH_CALLS.load(Ordering::SeqCst), 2);
        assert_eq!(DROP_DISPATCH_CALLS.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn structured_task_scope_normal_join_releases_all_children_exactly_once() {
        let _guard = TEST_GUARD.lock().expect("test guard mutex poisoned");
        let _executor_guard = executor::EXECUTOR_TEST_GUARD
            .lock()
            .expect("executor test guard mutex poisoned");
        executor::shutdown(executor::ConcurrentShutdownMode::Cancel);
        DROP_DISPATCH_CALLS.store(0, Ordering::SeqCst);
        assert_eq!(sengoo_async_runtime_enable_executor(2, 4), 1);

        let scope = sengoo_async_task_scope_new();
        assert!(scope > 0);
        for _ in 0..2 {
            assert_eq!(
                sengoo_async_task_scope_spawn_raw(
                    scope,
                    async_spawn_kind_id_for_tests(),
                    sengoo_async_sleep__start(5),
                ),
                1
            );
        }
        assert_eq!(sengoo_async_task_scope_join(scope), 2);
        assert_eq!(DROP_DISPATCH_CALLS.load(Ordering::SeqCst), 2);
        assert_eq!(task_scope::active_scope_count(), 0);
        assert_eq!(sengoo_async_task_scope_join(scope), 0);

        assert_eq!(sengoo_async_runtime_shutdown_executor(false), 0);
    }

    #[test]
    fn structured_task_scope_early_exit_cancels_then_joins_children() {
        let _guard = TEST_GUARD.lock().expect("test guard mutex poisoned");
        let _executor_guard = executor::EXECUTOR_TEST_GUARD
            .lock()
            .expect("executor test guard mutex poisoned");
        executor::shutdown(executor::ConcurrentShutdownMode::Cancel);
        CANCEL_DISPATCH_CALLS.store(0, Ordering::SeqCst);
        DROP_DISPATCH_CALLS.store(0, Ordering::SeqCst);
        assert_eq!(sengoo_async_runtime_enable_executor(2, 4), 1);

        let scope = sengoo_async_task_scope_new();
        for _ in 0..2 {
            assert_eq!(
                sengoo_async_task_scope_spawn_raw(
                    scope,
                    async_spawn_kind_id_for_tests(),
                    sengoo_async_sleep__start(1_000),
                ),
                1
            );
        }
        assert_eq!(sengoo_async_task_scope_cancel_join(scope), 3);
        assert_eq!(CANCEL_DISPATCH_CALLS.load(Ordering::SeqCst), 2);
        assert_eq!(DROP_DISPATCH_CALLS.load(Ordering::SeqCst), 0);
        assert_eq!(task_scope::active_scope_count(), 0);

        assert_eq!(sengoo_async_runtime_shutdown_executor(false), 0);
    }

    #[test]
    fn structured_task_scope_rejected_submission_releases_future_frame() {
        let _guard = TEST_GUARD.lock().expect("test guard mutex poisoned");
        CANCEL_DISPATCH_CALLS.store(0, Ordering::SeqCst);
        DROP_DISPATCH_CALLS.store(0, Ordering::SeqCst);

        assert_eq!(
            sengoo_async_task_scope_spawn_raw(
                i64::MAX,
                async_spawn_kind_id_for_tests(),
                sengoo_async_sleep__start(1_000),
            ),
            0
        );
        assert_eq!(CANCEL_DISPATCH_CALLS.load(Ordering::SeqCst), 1);
        assert_eq!(DROP_DISPATCH_CALLS.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn structured_task_scope_stress_leaves_no_scope_or_executor_tasks() {
        let _guard = TEST_GUARD.lock().expect("test guard mutex poisoned");
        let _executor_guard = executor::EXECUTOR_TEST_GUARD
            .lock()
            .expect("executor test guard mutex poisoned");
        executor::shutdown(executor::ConcurrentShutdownMode::Cancel);
        assert_eq!(sengoo_async_runtime_enable_executor(4, 8), 1);

        for _ in 0..100 {
            let scope = sengoo_async_task_scope_new();
            for _ in 0..4 {
                assert_eq!(
                    sengoo_async_task_scope_spawn_raw(
                        scope,
                        async_spawn_kind_id_for_tests(),
                        sengoo_async_sleep__start(0),
                    ),
                    1
                );
            }
            assert_eq!(sengoo_async_task_scope_join(scope), 2);
        }
        assert_eq!(task_scope::active_scope_count(), 0);
        assert_eq!(executor::active_task_count(), 0);

        assert_eq!(sengoo_async_runtime_shutdown_executor(false), 0);
    }

    #[test]
    fn concurrent_spawn_blocking_runs_on_worker_thread() {
        let _guard = TEST_GUARD.lock().expect("test guard mutex poisoned");
        thread_pool::test_only_disable_thread_pool();
        assert_eq!(sengoo_async_runtime_enable_thread_pool(2), 1);
        static WORKER_FLAG: AtomicI64 = AtomicI64::new(0);

        extern "C" fn worker() -> i64 {
            WORKER_FLAG.store(1, Ordering::SeqCst);
            77
        }

        let handle = sengoo_async_spawn_blocking_i64__start(worker);
        assert_ne!(handle, 0);
        while unsafe { sengoo_async_spawn_blocking_i64__poll(handle) } == 0 {
            std::thread::yield_now();
        }
        let value = unsafe { sengoo_async_spawn_blocking_i64__result(handle) };
        assert_eq!(value, 77);
        assert_eq!(WORKER_FLAG.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn concurrent_spawn_blocking_start_fails_without_pool() {
        let _guard = TEST_GUARD.lock().expect("test guard mutex poisoned");
        thread_pool::test_only_disable_thread_pool();
        extern "C" fn worker() -> i64 {
            1
        }
        assert_eq!(sengoo_async_spawn_blocking_i64__start(worker), 0);
    }

    #[test]
    fn concurrent_executor_steals_work_from_a_busy_worker_queue() {
        let _guard = TEST_GUARD.lock().expect("test guard mutex poisoned");
        thread_pool::test_only_disable_thread_pool();
        assert_eq!(sengoo_async_runtime_enable_thread_pool(2), 1);

        let blocker_started = Arc::new(AtomicBool::new(false));
        let release_blocker = Arc::new(AtomicBool::new(false));
        let blocker = thread_pool::test_only_submit_to_worker(
            {
                let blocker_started = blocker_started.clone();
                let release_blocker = release_blocker.clone();
                move || {
                    blocker_started.store(true, Ordering::Release);
                    while !release_blocker.load(Ordering::Acquire) {
                        std::thread::yield_now();
                    }
                    1
                }
            },
            0,
        )
        .expect("blocking job should be accepted");

        while !blocker_started.load(Ordering::Acquire) {
            std::thread::yield_now();
        }

        let mut stolen_jobs = Vec::new();
        for value in 2..=5 {
            stolen_jobs.push(
                thread_pool::test_only_submit_to_worker(move || value, 0)
                    .expect("queued job should be accepted"),
            );
        }
        for job in &stolen_jobs {
            while !job.completed.load(Ordering::Acquire) {
                std::thread::yield_now();
            }
        }

        assert!(
            thread_pool::test_only_steal_count() > 0,
            "an idle worker should steal jobs from the busy worker queue"
        );
        assert_eq!(
            stolen_jobs
                .iter()
                .map(|job| job.result.lock().expect("job result poisoned").unwrap_or(0))
                .sum::<i64>(),
            14
        );

        release_blocker.store(true, Ordering::Release);
        while !blocker.completed.load(Ordering::Acquire) {
            std::thread::yield_now();
        }
        thread_pool::test_only_disable_thread_pool();
        assert!(!sengoo_async_runtime_thread_pool_enabled());
    }

    #[test]
    fn concurrent_shared_counter_joins_workers_deterministically() {
        let _guard = TEST_GUARD.lock().expect("test guard mutex poisoned");
        thread_pool::test_only_disable_thread_pool();
        assert_eq!(sengoo_async_runtime_enable_thread_pool(4), 1);

        let counter = concurrent::sengoo_async_shared_counter_new_i64(2);
        assert_ne!(counter, 0);
        let mut jobs = Vec::new();
        for _ in 0..8 {
            let job =
                unsafe { concurrent::sengoo_async_shared_counter_spawn_add_i64(counter, 1, 5) };
            assert_ne!(job, 0);
            jobs.push(job);
        }
        for job in jobs {
            assert!(unsafe { concurrent::sengoo_async_shared_counter_join_i64(job) } >= 2);
            unsafe { concurrent::sengoo_async_shared_counter_job_drop(job) };
        }

        assert_eq!(
            unsafe { concurrent::sengoo_async_shared_counter_get_i64(counter) },
            42
        );
        unsafe { concurrent::sengoo_async_shared_counter_drop(counter) };
    }

    #[test]
    fn concurrent_generic_arc_mutex_payload_drops_exactly_once() {
        let _guard = TEST_GUARD.lock().expect("test guard mutex poisoned");
        thread_pool::test_only_disable_thread_pool();

        static MOVE_COUNT: AtomicU32 = AtomicU32::new(0);
        static DROP_COUNT: AtomicU32 = AtomicU32::new(0);

        extern "C" fn move_payload(
            destination: *mut std::ffi::c_void,
            source: *mut std::ffi::c_void,
        ) {
            MOVE_COUNT.fetch_add(1, Ordering::SeqCst);
            unsafe {
                std::ptr::copy_nonoverlapping(source.cast::<u8>(), destination.cast::<u8>(), 8);
                std::ptr::write_bytes(source.cast::<u8>(), 0, 8);
            }
        }

        extern "C" fn drop_payload(_value: *mut std::ffi::c_void) {
            DROP_COUNT.fetch_add(1, Ordering::SeqCst);
        }

        let descriptor = concurrent::SengooTypeDescriptor {
            abi_version: concurrent::SENGOO_COLLECTIONS_ABI_VERSION,
            flags: 0,
            size: 8,
            align: 8,
            move_value: Some(move_payload),
            drop_value: Some(drop_payload),
            clone_value: None,
            hash_value: None,
            eq_value: None,
            compare_value: None,
        };
        let mut payload = 41_i64;
        let arc = unsafe {
            concurrent::sengoo_arc_new(
                &descriptor,
                (&mut payload as *mut i64).cast::<std::ffi::c_void>(),
            )
        };
        assert_ne!(arc, 0);
        let cloned = unsafe { concurrent::sengoo_arc_clone(arc) };
        assert_ne!(cloned, 0);
        unsafe { concurrent::sengoo_arc_drop(cloned) };
        unsafe { concurrent::sengoo_arc_drop(arc) };

        assert_eq!(MOVE_COUNT.load(Ordering::SeqCst), 1);
        assert_eq!(DROP_COUNT.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn concurrent_generic_parts_accept_typed_callbacks_and_reject_missing_callbacks() {
        let _guard = TEST_GUARD.lock().expect("test guard mutex poisoned");
        thread_pool::test_only_disable_thread_pool();

        static MOVE_COUNT: AtomicU32 = AtomicU32::new(0);
        static DROP_COUNT: AtomicU32 = AtomicU32::new(0);

        extern "C" fn move_i64(destination: *mut std::ffi::c_void, source: *mut std::ffi::c_void) {
            MOVE_COUNT.fetch_add(1, Ordering::SeqCst);
            unsafe {
                std::ptr::copy_nonoverlapping(source.cast::<u8>(), destination.cast::<u8>(), 8);
                std::ptr::write_bytes(source.cast::<u8>(), 0, 8);
            }
        }

        extern "C" fn drop_i64(_value: *mut std::ffi::c_void) {
            DROP_COUNT.fetch_add(1, Ordering::SeqCst);
        }

        MOVE_COUNT.store(0, Ordering::SeqCst);
        DROP_COUNT.store(0, Ordering::SeqCst);

        let mut arc_value = 17_i64;
        let arc = unsafe {
            concurrent::sengoo_arc_new_parts(
                (&mut arc_value as *mut i64).cast(),
                8,
                8,
                Some(move_i64),
                Some(drop_i64),
            )
        };
        assert_ne!(arc, 0);
        assert_eq!(
            unsafe { *(concurrent::sengoo_arc_borrow_ptr(arc).cast::<i64>()) },
            17
        );
        unsafe { concurrent::sengoo_arc_drop(arc) };

        let mut mutex_value = 29_i64;
        let mutex = unsafe {
            concurrent::sengoo_async_mutex_new_parts(
                (&mut mutex_value as *mut i64).cast(),
                8,
                8,
                Some(move_i64),
                Some(drop_i64),
            )
        };
        assert_ne!(mutex, 0);
        let lock = concurrent::sengoo_async_mutex_lock__start(mutex);
        assert_ne!(lock, 0);
        assert_eq!(
            unsafe { concurrent::sengoo_async_mutex_lock__poll(lock) },
            1
        );
        assert_eq!(
            unsafe { concurrent::sengoo_async_mutex_lock__result(lock) },
            0
        );
        let mut copied = 0_i64;
        assert_eq!(
            unsafe {
                concurrent::sengoo_async_mutex_guard_copy_into(
                    mutex,
                    (&mut copied as *mut i64).cast(),
                    8,
                )
            },
            0
        );
        assert_eq!(copied, 29);

        let mut replacement = 31_i64;
        assert_eq!(
            unsafe {
                concurrent::sengoo_async_mutex_guard_set(
                    mutex,
                    (&mut replacement as *mut i64).cast(),
                    Some(drop_i64),
                )
            },
            0
        );
        let mut replaced = 0_i64;
        assert_eq!(
            unsafe {
                concurrent::sengoo_async_mutex_guard_copy_into(
                    mutex,
                    (&mut replaced as *mut i64).cast(),
                    8,
                )
            },
            0
        );
        assert_eq!(replaced, 31);
        assert_eq!(
            unsafe { concurrent::sengoo_async_mutex_guard_unlock(mutex) },
            0
        );

        let mut untouched = 9_i64;
        assert_eq!(
            unsafe {
                concurrent::sengoo_async_mutex_guard_copy_into(
                    0,
                    (&mut untouched as *mut i64).cast(),
                    8,
                )
            },
            -concurrent::STATUS_INVALID_HANDLE
        );
        assert_eq!(untouched, 9);

        let mut failed_replacement = 41_i64;
        assert_eq!(
            unsafe {
                concurrent::sengoo_async_mutex_guard_set(
                    0,
                    (&mut failed_replacement as *mut i64).cast(),
                    Some(drop_i64),
                )
            },
            -concurrent::STATUS_INVALID_HANDLE
        );
        unsafe { concurrent::sengoo_async_mutex_drop(mutex) };

        let mut rejected = 1_i64;
        assert_eq!(
            unsafe {
                concurrent::sengoo_arc_new_parts(
                    (&mut rejected as *mut i64).cast(),
                    8,
                    8,
                    None,
                    Some(drop_i64),
                )
            },
            0
        );
        assert_eq!(MOVE_COUNT.load(Ordering::SeqCst), 3);
        assert_eq!(DROP_COUNT.load(Ordering::SeqCst), 4);
    }

    #[test]
    fn concurrent_generic_arc_mutex_shared_counter_joins_workers_deterministically() {
        let _guard = TEST_GUARD.lock().expect("test guard mutex poisoned");
        thread_pool::test_only_disable_thread_pool();
        assert_eq!(sengoo_async_runtime_enable_thread_pool(4), 1);

        extern "C" fn move_i64(destination: *mut std::ffi::c_void, source: *mut std::ffi::c_void) {
            unsafe {
                std::ptr::copy_nonoverlapping(source.cast::<u8>(), destination.cast::<u8>(), 8);
                std::ptr::write_bytes(source.cast::<u8>(), 0, 8);
            }
        }

        extern "C" fn drop_i64(_value: *mut std::ffi::c_void) {}

        let descriptor = concurrent::SengooTypeDescriptor {
            abi_version: concurrent::SENGOO_COLLECTIONS_ABI_VERSION,
            flags: 0,
            size: 8,
            align: 8,
            move_value: Some(move_i64),
            drop_value: Some(drop_i64),
            clone_value: None,
            hash_value: None,
            eq_value: None,
            compare_value: None,
        };
        let mut initial = 2_i64;
        let mut mutex = unsafe {
            concurrent::sengoo_async_mutex_new(
                &descriptor,
                (&mut initial as *mut i64).cast::<std::ffi::c_void>(),
            )
        };
        assert_ne!(mutex, 0);
        let arc = unsafe {
            concurrent::sengoo_arc_new(
                &descriptor,
                (&mut mutex as *mut i64).cast::<std::ffi::c_void>(),
            )
        };
        assert_ne!(arc, 0);

        let mut jobs = Vec::new();
        for _ in 0..8 {
            let cloned = unsafe { concurrent::sengoo_arc_clone(arc) };
            let job =
                unsafe { concurrent::sengoo_async_shared_counter_spawn_add_i64(cloned, 1, 5) };
            assert_ne!(job, 0);
            jobs.push((cloned, job));
        }
        for (cloned, job) in jobs {
            assert!(unsafe { concurrent::sengoo_async_shared_counter_join_i64(job) } >= 2);
            unsafe { concurrent::sengoo_async_shared_counter_job_drop(job) };
            unsafe { concurrent::sengoo_arc_drop(cloned) };
        }

        let mutex_handle = unsafe { *(concurrent::sengoo_arc_borrow_ptr(arc).cast::<i64>()) };
        let locked = concurrent::sengoo_async_mutex_lock__start(mutex_handle);
        while unsafe { concurrent::sengoo_async_mutex_lock__poll(locked) } == 0 {
            std::thread::yield_now();
        }
        assert_eq!(
            unsafe { concurrent::sengoo_async_mutex_lock__result(locked) },
            0
        );
        let value_ptr = unsafe { concurrent::sengoo_async_mutex_guard_get(mutex_handle) };
        assert!(!value_ptr.is_null());
        assert_eq!(unsafe { *(value_ptr.cast::<i64>()) }, 42);
        assert_eq!(
            unsafe { concurrent::sengoo_async_mutex_guard_unlock(mutex_handle) },
            0
        );

        unsafe { concurrent::sengoo_arc_drop(arc) };
    }

    #[test]
    fn concurrent_shared_counter_job_keeps_last_source_arc_alive() {
        let _guard = TEST_GUARD.lock().expect("test guard mutex poisoned");
        thread_pool::test_only_disable_thread_pool();
        assert_eq!(sengoo_async_runtime_enable_thread_pool(1), 1);

        let release = Arc::new(AtomicBool::new(false));
        let worker_release = release.clone();
        let blocker = thread_pool::test_only_submit_to_worker(
            move || {
                while !worker_release.load(Ordering::Acquire) {
                    std::thread::yield_now();
                }
                0
            },
            0,
        )
        .expect("blocker should occupy the only worker");

        let counter = concurrent::sengoo_async_shared_counter_new_i64(2);
        assert_ne!(counter, 0);
        let job = unsafe { concurrent::sengoo_async_shared_counter_spawn_add_i64(counter, 1, 40) };
        assert_ne!(job, 0);
        unsafe { concurrent::sengoo_async_shared_counter_drop(counter) };

        release.store(true, Ordering::Release);
        while !blocker.completed.load(Ordering::Acquire) {
            std::thread::yield_now();
        }
        assert_eq!(
            unsafe { concurrent::sengoo_async_shared_counter_join_i64(job) },
            42
        );
        unsafe { concurrent::sengoo_async_shared_counter_job_drop(job) };
    }

    #[test]
    fn concurrent_channel_send_recv_round_trip() {
        let _guard = TEST_GUARD.lock().expect("test guard mutex poisoned");
        thread_pool::test_only_disable_thread_pool();
        let pair = sengoo_async_channel_bounded_i64(2);
        let sender = unsafe { sengoo_async_channel_pair_sender(pair) };
        let receiver = unsafe { sengoo_async_channel_pair_receiver(pair) };

        let send_handle = sengoo_async_channel_send_i64__start(sender, 41);
        while unsafe { sengoo_async_channel_send_i64__poll(send_handle) } == 0 {}
        let send_result = unsafe { sengoo_async_channel_send_i64__result(send_handle) };
        assert!(send_result.is_ok);

        let recv_handle = sengoo_async_channel_recv_i64__start(receiver);
        while unsafe { sengoo_async_channel_recv_i64__poll(recv_handle) } == 0 {}
        let recv_result = unsafe { sengoo_async_channel_recv_i64__result(recv_handle) };
        assert!(recv_result.is_ok);
        assert_eq!(recv_result.value, 41);

        unsafe { sengoo_async_channel_pair_free(pair) };
    }

    #[test]
    fn concurrent_channel_close_wakes_pending_receiver() {
        let _guard = TEST_GUARD.lock().expect("test guard mutex poisoned");
        thread_pool::test_only_disable_thread_pool();
        let pair = sengoo_async_channel_bounded_i64(1);
        let sender = unsafe { sengoo_async_channel_pair_sender(pair) };
        let receiver = unsafe { sengoo_async_channel_pair_receiver(pair) };

        let recv_handle = sengoo_async_channel_recv_i64__start(receiver);
        assert_eq!(
            unsafe { sengoo_async_channel_recv_i64__poll(recv_handle) },
            0
        );
        unsafe { sengoo_async_channel_sender_close(sender) };
        assert_eq!(
            unsafe { sengoo_async_channel_recv_i64__poll(recv_handle) },
            1
        );
        let recv_result = unsafe { sengoo_async_channel_recv_i64__result(recv_handle) };
        assert!(!recv_result.is_ok);
        assert_eq!(recv_result.error, concurrent::STATUS_INVALID_HANDLE);

        unsafe { sengoo_async_channel_pair_free(pair) };
    }

    #[test]
    fn concurrent_mutex_lock_and_unlock_round_trip() {
        let _guard = TEST_GUARD.lock().expect("test guard mutex poisoned");
        thread_pool::test_only_disable_thread_pool();
        let mutex = sengoo_async_mutex_new_i64(5);
        let lock_handle = sengoo_async_mutex_lock_i64__start(mutex);
        while unsafe { sengoo_async_mutex_lock_i64__poll(lock_handle) } == 0 {}
        let lock_result = unsafe { sengoo_async_mutex_lock_i64__result(lock_handle) };
        assert!(lock_result.is_ok);
        assert_eq!(lock_result.value, 5);
        assert_eq!(unsafe { sengoo_async_mutex_unlock_i64(mutex, 9) }, 0);
        unsafe { sengoo_async_mutex_drop(mutex) };
    }

    #[test]
    fn concurrent_mutex_rejects_double_unlock_without_corrupting_next_lock() {
        let _guard = TEST_GUARD.lock().expect("test guard mutex poisoned");
        thread_pool::test_only_disable_thread_pool();
        let mutex = sengoo_async_mutex_new_i64(5);
        let first_lock = sengoo_async_mutex_lock_i64__start(mutex);
        while unsafe { sengoo_async_mutex_lock_i64__poll(first_lock) } == 0 {}
        let first_result = unsafe { sengoo_async_mutex_lock_i64__result(first_lock) };
        assert!(first_result.is_ok);
        assert_eq!(unsafe { sengoo_async_mutex_guard_get_i64(mutex) }, 5);
        assert_eq!(unsafe { sengoo_async_mutex_guard_set_i64(mutex, 9) }, 0);
        assert_eq!(unsafe { sengoo_async_mutex_guard_get_i64(mutex) }, 9);
        assert_eq!(unsafe { sengoo_async_mutex_guard_unlock_i64(mutex) }, 0);
        assert_eq!(
            unsafe { sengoo_async_mutex_guard_unlock_i64(mutex) },
            -concurrent::STATUS_INVALID_HANDLE
        );

        let second_lock = sengoo_async_mutex_lock_i64__start(mutex);
        while unsafe { sengoo_async_mutex_lock_i64__poll(second_lock) } == 0 {}
        let second_result = unsafe { sengoo_async_mutex_lock_i64__result(second_lock) };
        assert!(second_result.is_ok);
        assert_eq!(second_result.value, 9);
        assert_eq!(unsafe { sengoo_async_mutex_unlock_i64(mutex, 11) }, 0);
        unsafe { sengoo_async_mutex_drop(mutex) };
    }

    #[test]
    fn concurrent_generic_rwlock_supports_multiple_readers_exclusive_writer_and_i64_wrappers() {
        let _guard = TEST_GUARD.lock().expect("test guard mutex poisoned");
        thread_pool::test_only_disable_thread_pool();

        extern "C" fn drop_i64(_value: *mut std::ffi::c_void) {}

        let lock = sengoo_async_rwlock_new_i64(5);

        let first_reader = unsafe { sengoo_async_rwlock_try_read(lock) };
        let second_reader = unsafe { sengoo_async_rwlock_try_read_i64(lock) };
        assert!(first_reader > 0);
        assert!(second_reader > 0);

        let mut copied = 0_i64;
        assert_eq!(
            unsafe {
                sengoo_async_rwlock_read_guard_copy_into(
                    lock,
                    first_reader,
                    (&mut copied as *mut i64).cast(),
                    8,
                )
            },
            0
        );
        assert_eq!(copied, 5);
        assert_eq!(
            unsafe { sengoo_async_rwlock_read_guard_get_i64(lock, second_reader) },
            5
        );
        assert_eq!(
            unsafe { sengoo_async_rwlock_try_write(lock) },
            -concurrent::STATUS_LOCK_UNAVAILABLE
        );

        assert_eq!(
            unsafe { sengoo_async_rwlock_read_guard_unlock(lock, first_reader) },
            0
        );
        assert_eq!(
            unsafe { sengoo_async_rwlock_read_guard_unlock_i64(lock, first_reader) },
            -concurrent::STATUS_INVALID_HANDLE
        );
        assert_eq!(
            unsafe { sengoo_async_rwlock_try_write_i64(lock) },
            -concurrent::STATUS_LOCK_UNAVAILABLE
        );
        assert_eq!(
            unsafe { sengoo_async_rwlock_read_guard_unlock_i64(lock, second_reader) },
            0
        );

        let writer = unsafe { sengoo_async_rwlock_try_write_i64(lock) };
        assert!(writer > 0);
        let mut replacement = 9_i64;
        assert_eq!(
            unsafe {
                sengoo_async_rwlock_write_guard_set(
                    lock,
                    writer,
                    (&mut replacement as *mut i64).cast(),
                    Some(drop_i64),
                )
            },
            0
        );
        assert_eq!(
            unsafe { sengoo_async_rwlock_write_guard_get_i64(lock, writer) },
            9
        );
        assert_eq!(
            unsafe { sengoo_async_rwlock_try_read_i64(lock) },
            -concurrent::STATUS_LOCK_UNAVAILABLE
        );
        assert_eq!(
            unsafe { sengoo_async_rwlock_write_guard_unlock(lock, writer) },
            0
        );
        assert_eq!(
            unsafe { sengoo_async_rwlock_write_guard_unlock_i64(lock, writer) },
            -concurrent::STATUS_INVALID_HANDLE
        );

        let final_reader = unsafe { sengoo_async_rwlock_try_read_i64(lock) };
        assert!(final_reader > 0);
        assert_eq!(
            unsafe { sengoo_async_rwlock_read_guard_get_i64(lock, final_reader) },
            9
        );
        assert_eq!(
            unsafe { sengoo_async_rwlock_read_guard_unlock_i64(lock, final_reader) },
            0
        );
        unsafe { sengoo_async_rwlock_close(lock) };
        assert_eq!(
            unsafe { sengoo_async_rwlock_try_read(lock) },
            -concurrent::STATUS_INVALID_HANDLE
        );
        unsafe { sengoo_async_rwlock_drop(lock) };
    }

    #[test]
    fn concurrent_generic_rwlock_invalid_copy_and_set_preserve_output_and_consume_inputs() {
        let _guard = TEST_GUARD.lock().expect("test guard mutex poisoned");
        thread_pool::test_only_disable_thread_pool();

        static MOVE_COUNT: AtomicU32 = AtomicU32::new(0);
        static DROP_COUNT: AtomicU32 = AtomicU32::new(0);

        extern "C" fn move_i64(destination: *mut std::ffi::c_void, source: *mut std::ffi::c_void) {
            MOVE_COUNT.fetch_add(1, Ordering::SeqCst);
            unsafe {
                std::ptr::copy_nonoverlapping(source.cast::<u8>(), destination.cast::<u8>(), 8);
                std::ptr::write_bytes(source.cast::<u8>(), 0, 8);
            }
        }

        extern "C" fn drop_i64(_value: *mut std::ffi::c_void) {
            DROP_COUNT.fetch_add(1, Ordering::SeqCst);
        }

        MOVE_COUNT.store(0, Ordering::SeqCst);
        DROP_COUNT.store(0, Ordering::SeqCst);

        let mut initial = 7_i64;
        let lock = unsafe {
            sengoo_async_rwlock_new_parts(
                (&mut initial as *mut i64).cast(),
                8,
                8,
                Some(move_i64),
                Some(drop_i64),
            )
        };
        assert_ne!(lock, 0);

        let reader = unsafe { sengoo_async_rwlock_try_read(lock) };
        assert!(reader > 0);

        let mut read_output = 123_i64;
        assert_eq!(
            unsafe {
                sengoo_async_rwlock_read_guard_copy_into(
                    lock,
                    reader,
                    (&mut read_output as *mut i64).cast(),
                    4,
                )
            },
            -concurrent::STATUS_INVALID_ARGUMENT
        );
        assert_eq!(read_output, 123);
        assert_eq!(
            unsafe {
                sengoo_async_rwlock_read_guard_copy_into(
                    lock,
                    0,
                    (&mut read_output as *mut i64).cast(),
                    8,
                )
            },
            -concurrent::STATUS_INVALID_HANDLE
        );
        assert_eq!(read_output, 123);
        assert_eq!(
            unsafe { sengoo_async_rwlock_read_guard_unlock(lock, reader) },
            0
        );
        assert_eq!(
            unsafe {
                sengoo_async_rwlock_read_guard_copy_into(
                    lock,
                    reader,
                    (&mut read_output as *mut i64).cast(),
                    8,
                )
            },
            -concurrent::STATUS_INVALID_HANDLE
        );
        assert_eq!(read_output, 123);

        let writer = unsafe { sengoo_async_rwlock_try_write(lock) };
        assert!(writer > 0);

        let mut rejected = 11_i64;
        assert_eq!(
            unsafe {
                sengoo_async_rwlock_write_guard_set(
                    lock,
                    0,
                    (&mut rejected as *mut i64).cast(),
                    Some(drop_i64),
                )
            },
            -concurrent::STATUS_INVALID_HANDLE
        );
        assert_eq!(DROP_COUNT.load(Ordering::SeqCst), 1);

        let mut write_output = 456_i64;
        assert_eq!(
            unsafe {
                sengoo_async_rwlock_write_guard_copy_into(
                    lock,
                    writer,
                    (&mut write_output as *mut i64).cast(),
                    8,
                )
            },
            0
        );
        assert_eq!(write_output, 7);

        write_output = 789;
        assert_eq!(
            unsafe {
                sengoo_async_rwlock_write_guard_copy_into(
                    lock,
                    writer,
                    (&mut write_output as *mut i64).cast(),
                    4,
                )
            },
            -concurrent::STATUS_INVALID_ARGUMENT
        );
        assert_eq!(write_output, 789);
        assert_eq!(
            unsafe {
                sengoo_async_rwlock_write_guard_copy_into(
                    lock,
                    999_999,
                    (&mut write_output as *mut i64).cast(),
                    8,
                )
            },
            -concurrent::STATUS_INVALID_HANDLE
        );
        assert_eq!(write_output, 789);
        assert_eq!(
            unsafe { sengoo_async_rwlock_write_guard_unlock(lock, writer) },
            0
        );

        unsafe { sengoo_async_rwlock_drop(lock) };
        assert_eq!(MOVE_COUNT.load(Ordering::SeqCst), 1);
        assert_eq!(DROP_COUNT.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn concurrent_generic_rwlock_close_and_drop_stay_stable_and_drop_once() {
        let _guard = TEST_GUARD.lock().expect("test guard mutex poisoned");
        thread_pool::test_only_disable_thread_pool();

        static MOVE_COUNT: AtomicU32 = AtomicU32::new(0);
        static DROP_COUNT: AtomicU32 = AtomicU32::new(0);

        extern "C" fn move_i64(destination: *mut std::ffi::c_void, source: *mut std::ffi::c_void) {
            MOVE_COUNT.fetch_add(1, Ordering::SeqCst);
            unsafe {
                std::ptr::copy_nonoverlapping(source.cast::<u8>(), destination.cast::<u8>(), 8);
                std::ptr::write_bytes(source.cast::<u8>(), 0, 8);
            }
        }

        extern "C" fn drop_i64(_value: *mut std::ffi::c_void) {
            DROP_COUNT.fetch_add(1, Ordering::SeqCst);
        }

        MOVE_COUNT.store(0, Ordering::SeqCst);
        DROP_COUNT.store(0, Ordering::SeqCst);

        let mut initial = 5_i64;
        let lock = unsafe {
            sengoo_async_rwlock_new_parts(
                (&mut initial as *mut i64).cast(),
                8,
                8,
                Some(move_i64),
                Some(drop_i64),
            )
        };
        assert_ne!(lock, 0);
        assert_eq!(MOVE_COUNT.load(Ordering::SeqCst), 1);
        assert_eq!(DROP_COUNT.load(Ordering::SeqCst), 0);

        let writer = unsafe { sengoo_async_rwlock_try_write(lock) };
        assert!(writer > 0);

        let mut replacement = 13_i64;
        assert_eq!(
            unsafe {
                sengoo_async_rwlock_write_guard_set(
                    lock,
                    writer,
                    (&mut replacement as *mut i64).cast(),
                    Some(drop_i64),
                )
            },
            0
        );
        assert_eq!(MOVE_COUNT.load(Ordering::SeqCst), 2);
        assert_eq!(DROP_COUNT.load(Ordering::SeqCst), 1);
        assert_eq!(
            unsafe { sengoo_async_rwlock_write_guard_unlock(lock, writer) },
            0
        );

        unsafe { sengoo_async_rwlock_close(lock) };
        unsafe { sengoo_async_rwlock_close(lock) };
        assert_eq!(DROP_COUNT.load(Ordering::SeqCst), 1);
        unsafe { sengoo_async_rwlock_drop(lock) };
        assert_eq!(DROP_COUNT.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn concurrent_canceled_blocking_future_discards_worker_result() {
        let _guard = TEST_GUARD.lock().expect("test guard mutex poisoned");
        thread_pool::test_only_disable_thread_pool();
        assert_eq!(sengoo_async_runtime_enable_thread_pool(1), 1);

        extern "C" fn slow_worker() -> i64 {
            std::thread::sleep(Duration::from_millis(30));
            99
        }

        let handle = sengoo_async_spawn_blocking_i64__start(slow_worker);
        assert_ne!(handle, 0);
        let canceled = unsafe { sengoo_async_spawn_blocking_i64__cancel(handle) };
        assert!(
            canceled,
            "blocking future cancel should consume pending handle"
        );
        std::thread::sleep(Duration::from_millis(40));
    }
}
