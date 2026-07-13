use std::cell::Cell;
use std::ptr::NonNull;
use std::sync::{Arc, Mutex};

use super::{
    concurrent, executor, main__poll, main__result, main__start, sengoo_async_cancel_dispatch,
    sengoo_async_drop_dispatch, sengoo_async_poll_dispatch, CoroutineScheduler, CoroutineTask,
    TaskId, TaskLifecycleStatus, TaskState,
};

thread_local! {
    pub(super) static CURRENT_SCHEDULER: Cell<*mut CoroutineScheduler> = const { Cell::new(std::ptr::null_mut()) };
}

fn scheduler_nonnull(ptr: *mut CoroutineScheduler) -> Option<NonNull<CoroutineScheduler>> {
    NonNull::new(ptr)
}

pub(super) unsafe fn scheduler_mut<'a>(
    ptr: *mut CoroutineScheduler,
) -> Option<&'a mut CoroutineScheduler> {
    scheduler_nonnull(ptr).map(|mut ptr| ptr.as_mut())
}

struct RootAsyncMainI64Task {
    handle: Option<i64>,
    result: Arc<Mutex<Option<i64>>>,
}

impl RootAsyncMainI64Task {
    fn new(result: Arc<Mutex<Option<i64>>>) -> Self {
        Self {
            handle: None,
            result,
        }
    }
}

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

pub(super) struct ForeignAsyncTask {
    pub(super) kind: i64,
    pub(super) handle: i64,
}

impl CoroutineTask for ForeignAsyncTask {
    fn foreign_identity(&self) -> Option<(i64, i64)> {
        if self.handle == 0 {
            None
        } else {
            Some((self.kind, self.handle))
        }
    }

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

pub(super) fn cancel_scheduled_foreign(kind: i64, handle: i64) -> bool {
    CURRENT_SCHEDULER.with(|cell| {
        let scheduler = cell.get();
        let Some(scheduler) = (unsafe { scheduler_mut(scheduler) }) else {
            return false;
        };
        scheduler.cancel_foreign(kind, handle)
    })
}

#[no_mangle]
pub extern "C" fn sengoo_async_scheduler_new() -> *mut CoroutineScheduler {
    Box::into_raw(Box::new(CoroutineScheduler::new()))
}

#[no_mangle]
/// # Safety
///
/// `scheduler` must be null or a live pointer returned by
/// [`sengoo_async_scheduler_new`] that has not already been freed.
pub unsafe extern "C" fn sengoo_async_scheduler_free(scheduler: *mut CoroutineScheduler) {
    let Some(scheduler) = scheduler_nonnull(scheduler) else {
        return;
    };
    drop(Box::from_raw(scheduler.as_ptr()));
}

#[no_mangle]
/// # Safety
///
/// `scheduler` must be null or a live pointer returned by
/// [`sengoo_async_scheduler_new`].
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

#[no_mangle]
/// # Safety
///
/// `scheduler` must be null or a live pointer returned by
/// [`sengoo_async_scheduler_new`].
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

#[no_mangle]
/// # Safety
///
/// `scheduler` must be null or a live pointer returned by
/// [`sengoo_async_scheduler_new`].
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

#[no_mangle]
pub extern "C" fn sengoo_async_cancel_task(task_id: i64) -> bool {
    if executor::is_concurrent_task_id(task_id) {
        return executor::cancel(task_id);
    }
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

#[no_mangle]
pub extern "C" fn sengoo_async_task_status(task_id: i64) -> i64 {
    if executor::is_concurrent_task_id(task_id) {
        return executor::task_status(task_id) as i64;
    }
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

#[no_mangle]
pub extern "C" fn sengoo_async_spawn_raw(kind: i64, handle: i64) -> i64 {
    CURRENT_SCHEDULER.with(|cell| {
        let scheduler = cell.get();
        let Some(scheduler) = (unsafe { scheduler_mut(scheduler) }) else {
            return 0;
        };
        let task_id = scheduler.spawn(ForeignAsyncTask { kind, handle });
        let _ = scheduler.tick();
        task_id as i64
    })
}

pub(super) struct DetachedForeignAsyncTask {
    pub(super) kind: i64,
    pub(super) handle: i64,
}

impl CoroutineTask for DetachedForeignAsyncTask {
    fn foreign_identity(&self) -> Option<(i64, i64)> {
        (self.handle != 0).then_some((self.kind, self.handle))
    }

    fn poll(&mut self) -> TaskState {
        if unsafe { sengoo_async_poll_dispatch(self.kind, self.handle) } == 0 {
            return TaskState::Pending;
        }
        if self.handle != 0 {
            unsafe { sengoo_async_drop_dispatch(self.kind, self.handle) };
            self.handle = 0;
        }
        TaskState::Complete
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

impl DetachedForeignAsyncTask {
    pub(super) fn release_rejected(mut self) {
        if self.handle == 0 {
            return;
        }
        let canceled = unsafe { sengoo_async_cancel_dispatch(self.kind, self.handle) };
        if !canceled {
            unsafe { sengoo_async_drop_dispatch(self.kind, self.handle) };
        }
        self.handle = 0;
    }
}

#[no_mangle]
pub extern "C" fn sengoo_async_runtime_enable_executor(worker_count: i64, capacity: i64) -> i64 {
    executor::enable(worker_count, capacity).unwrap_or_else(|status| -status)
}

#[no_mangle]
pub extern "C" fn sengoo_async_runtime_executor_enabled() -> bool {
    executor::is_enabled()
}

#[no_mangle]
pub extern "C" fn sengoo_async_spawn_task_raw(kind: i64, handle: i64) -> i64 {
    if executor::is_enabled() {
        executor::spawn_detached(kind, handle)
    } else {
        CURRENT_SCHEDULER.with(|cell| {
            let scheduler = cell.get();
            let Some(scheduler) = (unsafe { scheduler_mut(scheduler) }) else {
                return 0;
            };
            let task_id = scheduler.spawn(DetachedForeignAsyncTask { kind, handle });
            let _ = scheduler.tick();
            task_id as i64
        })
    }
}

#[no_mangle]
pub extern "C" fn sengoo_async_join_task(task_id: i64) -> i64 {
    if executor::is_concurrent_task_id(task_id) {
        return executor::join(task_id) as i64;
    }
    CURRENT_SCHEDULER.with(|cell| {
        let scheduler = cell.get();
        let Some(scheduler) = (unsafe { scheduler_mut(scheduler) }) else {
            return TaskLifecycleStatus::Unknown as i64;
        };
        let Ok(task_id) = TaskId::try_from(task_id) else {
            return TaskLifecycleStatus::Unknown as i64;
        };
        if task_id == 0 {
            return TaskLifecycleStatus::Unknown as i64;
        }
        loop {
            let status = scheduler.task_status(task_id);
            if status != TaskLifecycleStatus::Pending {
                return status as i64;
            }
            let _ = scheduler.run_until_idle(1);
        }
    })
}

#[no_mangle]
pub extern "C" fn sengoo_async_runtime_shutdown_executor(cancel_pending: bool) -> i64 {
    let mode = if cancel_pending {
        executor::ConcurrentShutdownMode::Cancel
    } else {
        executor::ConcurrentShutdownMode::Drain
    };
    executor::shutdown(mode);
    0
}

#[no_mangle]
pub extern "C" fn sengoo_async_run_main_i64() -> i64 {
    concurrent::retain_native_bridge_exports_for_linker();
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

    executor::shutdown(executor::ConcurrentShutdownMode::Drain);

    let final_result = result
        .lock()
        .expect("async main result mutex poisoned")
        .unwrap_or_default();
    final_result
}
