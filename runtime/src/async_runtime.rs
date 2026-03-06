//! Coroutine-compatible async runtime scheduling primitives.

use std::collections::VecDeque;
#[cfg(feature = "native-bridge")]
use std::sync::{Arc, Mutex};

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

        for _ in 0..cycle_len {
            let Some(mut entry) = self.queue.pop_front() else {
                break;
            };

            match entry.task.poll() {
                TaskState::Pending => self.queue.push_back(entry),
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
        }

        finished
    }
}

#[cfg(feature = "native-bridge")]
unsafe extern "C" {
    fn main__start() -> i64;
    fn main__poll(handle: i64) -> i64;
    fn main__result(handle: i64) -> i64;
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
    (&mut *scheduler).run_until_idle(max_ticks).len()
}

#[cfg(feature = "native-bridge")]
#[no_mangle]
pub extern "C" fn sengoo_async_run_main_i64() -> i64 {
    let result = Arc::new(Mutex::new(None));
    let mut scheduler = CoroutineScheduler::new();
    scheduler.spawn(RootAsyncMainI64Task::new(result.clone()));

    while !scheduler.is_empty() {
        scheduler.tick();
    }

    let final_result = result
        .lock()
        .expect("async main result mutex poisoned")
        .unwrap_or_default();
    final_result
}

#[cfg(all(test, feature = "native-bridge"))]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    unsafe extern "C" {
        fn sengoo_async_run_main_i64() -> i64;
    }

    static TEST_POLLS: AtomicU32 = AtomicU32::new(0);

    #[no_mangle]
    pub extern "C" fn main__start() -> i64 {
        7
    }

    #[no_mangle]
    pub extern "C" fn main__poll(handle: i64) -> i64 {
        assert_eq!(handle, 7);
        let polls = TEST_POLLS.fetch_add(1, Ordering::SeqCst);
        if polls >= 2 { 1 } else { 0 }
    }

    #[no_mangle]
    pub extern "C" fn main__result(handle: i64) -> i64 {
        assert_eq!(handle, 7);
        42
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

    #[test]
    fn scheduler_completes_tasks_over_multiple_ticks() {
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
    fn ffi_bridge_runs_root_i64_task_to_completion() {
        TEST_POLLS.store(0, Ordering::SeqCst);
        let result = unsafe { sengoo_async_run_main_i64() };
        assert_eq!(result, 42);
        assert_eq!(TEST_POLLS.load(Ordering::SeqCst), 3);
    }
}