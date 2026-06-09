use std::sync::atomic::{AtomicU8, Ordering};
use std::time::{Duration, Instant};

use super::reactor::STATUS_TIMEOUT_CODE;
use super::{
    clear_poll_wakeup_hint, handle_mut, handle_ref, handle_take_box,
    merge_wakeup_hint_with_deadline, record_poll_wakeup_hint, sengoo_async_cancel_dispatch,
    sengoo_async_poll_dispatch, take_poll_wakeup_hint,
};

pub(super) const POLL_ERROR_REENTRANT: i64 = -2;
pub(super) const POLL_ERROR_COMPLETED: i64 = -3;

#[derive(Debug, Default)]
pub(super) struct PollLifecycle {
    state: AtomicU8,
}

pub(super) struct PollGuard<'a> {
    lifecycle: &'a PollLifecycle,
    ready: bool,
}

impl PollLifecycle {
    pub(super) fn enter(&self) -> Result<PollGuard<'_>, i64> {
        match self
            .state
            .compare_exchange(0, 1, Ordering::AcqRel, Ordering::Acquire)
        {
            Ok(_) => Ok(PollGuard {
                lifecycle: self,
                ready: false,
            }),
            Err(1) => Err(POLL_ERROR_REENTRANT),
            Err(_) => Err(POLL_ERROR_COMPLETED),
        }
    }
}

impl PollGuard<'_> {
    pub(super) fn mark_ready(mut self) {
        self.lifecycle.state.store(2, Ordering::Release);
        self.ready = true;
    }
}

impl Drop for PollGuard<'_> {
    fn drop(&mut self) {
        if !self.ready {
            self.lifecycle.state.store(0, Ordering::Release);
        }
    }
}

unsafe extern "C" {
    fn sengoo_async_result_dispatch_i64(kind: i64, handle: i64) -> i64;
}

#[derive(Debug)]
pub(super) struct SleepFutureState {
    pub(super) deadline: Instant,
    pub(super) lifecycle: PollLifecycle,
}

struct TimeoutBoolFutureState {
    child_kind: i64,
    child_handle: i64,
    deadline: Instant,
    result: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TimeoutCancelOutcome {
    Pending,
    Completed,
    TimedOut,
}

struct TimeoutCancelI64FutureState {
    child_kind: i64,
    child_handle: i64,
    deadline: Instant,
    outcome: TimeoutCancelOutcome,
    completed_value: i64,
}

fn sleep_duration(duration_ms: i64) -> Duration {
    Duration::from_millis(duration_ms.max(0) as u64)
}

#[no_mangle]
pub extern "C" fn sengoo_async_sleep__start(duration_ms: i64) -> i64 {
    let state = SleepFutureState {
        deadline: Instant::now() + sleep_duration(duration_ms),
        lifecycle: PollLifecycle::default(),
    };
    Box::into_raw(Box::new(state)) as i64
}

#[no_mangle]
/// # Safety
///
/// `handle` must be zero or a live handle returned by
/// [`sengoo_async_sleep__start`].
pub unsafe extern "C" fn sengoo_async_sleep__poll(handle: i64) -> i64 {
    let Some(state) = handle_ref::<SleepFutureState>(handle) else {
        return 1;
    };
    let guard = match state.lifecycle.enter() {
        Ok(guard) => guard,
        Err(error) => return error,
    };
    if Instant::now() >= state.deadline {
        guard.mark_ready();
        1
    } else {
        record_poll_wakeup_hint(state.deadline);
        0
    }
}

#[no_mangle]
/// # Safety
///
/// `handle` must be zero or an unconsumed handle returned by
/// [`sengoo_async_sleep__start`].
pub unsafe extern "C" fn sengoo_async_sleep__result(handle: i64) {
    let Some(state) = handle_take_box::<SleepFutureState>(handle) else {
        return;
    };
    drop(state);
}

#[no_mangle]
/// # Safety
///
/// `handle` must be zero or an unconsumed handle returned by
/// [`sengoo_async_sleep__start`].
pub unsafe extern "C" fn sengoo_async_sleep__cancel(handle: i64) -> bool {
    let Some(state) = handle_take_box::<SleepFutureState>(handle) else {
        return false;
    };
    drop(state);
    true
}

#[no_mangle]
/// # Safety
///
/// `handle` must be zero or an unconsumed handle returned by
/// [`sengoo_async_sleep__start`].
pub unsafe extern "C" fn sengoo_async_sleep__drop(handle: i64) {
    let Some(state) = handle_take_box::<SleepFutureState>(handle) else {
        return;
    };
    drop(state);
}

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

#[no_mangle]
/// # Safety
///
/// `handle` must be zero or a live handle returned by
/// [`sengoo_async_timeout_bool__start`].
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

#[no_mangle]
/// # Safety
///
/// `handle` must be zero or an unconsumed handle returned by
/// [`sengoo_async_timeout_bool__start`].
pub unsafe extern "C" fn sengoo_async_timeout_bool__result(handle: i64) -> bool {
    let Some(state) = handle_take_box::<TimeoutBoolFutureState>(handle) else {
        return false;
    };
    state.result.unwrap_or(false)
}

#[no_mangle]
/// # Safety
///
/// `handle` must be zero or an unconsumed handle returned by
/// [`sengoo_async_timeout_bool__start`].
pub unsafe extern "C" fn sengoo_async_timeout_bool__cancel(handle: i64) -> bool {
    let Some(state) = handle_take_box::<TimeoutBoolFutureState>(handle) else {
        return false;
    };
    drop(state);
    true
}

#[no_mangle]
/// # Safety
///
/// `handle` must be zero or an unconsumed handle returned by
/// [`sengoo_async_timeout_bool__start`].
pub unsafe extern "C" fn sengoo_async_timeout_bool__drop(handle: i64) {
    let Some(state) = handle_take_box::<TimeoutBoolFutureState>(handle) else {
        return;
    };
    drop(state);
}

#[no_mangle]
pub extern "C" fn sengoo_async_timeout_cancel_i64__start(
    child_kind: i64,
    child_handle: i64,
    duration_ms: i64,
) -> i64 {
    let state = TimeoutCancelI64FutureState {
        child_kind,
        child_handle,
        deadline: Instant::now() + sleep_duration(duration_ms),
        outcome: TimeoutCancelOutcome::Pending,
        completed_value: 0,
    };
    Box::into_raw(Box::new(state)) as i64
}

#[no_mangle]
/// # Safety
///
/// `handle` must be zero or a live handle returned by
/// [`sengoo_async_timeout_cancel_i64__start`].
pub unsafe extern "C" fn sengoo_async_timeout_cancel_i64__poll(handle: i64) -> i64 {
    let Some(state) = handle_mut::<TimeoutCancelI64FutureState>(handle) else {
        return 1;
    };
    if state.outcome != TimeoutCancelOutcome::Pending {
        return 1;
    }

    clear_poll_wakeup_hint();
    if sengoo_async_poll_dispatch(state.child_kind, state.child_handle) != 0 {
        state.completed_value =
            sengoo_async_result_dispatch_i64(state.child_kind, state.child_handle);
        state.outcome = TimeoutCancelOutcome::Completed;
        return 1;
    }
    let child_hint = take_poll_wakeup_hint();

    let now = Instant::now();
    if now >= state.deadline {
        let _ = sengoo_async_cancel_dispatch(state.child_kind, state.child_handle);
        state.outcome = TimeoutCancelOutcome::TimedOut;
        return 1;
    }

    record_poll_wakeup_hint(merge_wakeup_hint_with_deadline(child_hint, state.deadline));
    0
}

#[repr(C)]
pub struct TimeoutCancelI64Result {
    pub is_ok: bool,
    pub value: i64,
    pub error: i64,
}

#[no_mangle]
/// # Safety
///
/// `handle` must be zero or an unconsumed handle returned by
/// [`sengoo_async_timeout_cancel_i64__start`].
pub unsafe extern "C" fn sengoo_async_timeout_cancel_i64__result(
    handle: i64,
) -> TimeoutCancelI64Result {
    let Some(state) = handle_take_box::<TimeoutCancelI64FutureState>(handle) else {
        return TimeoutCancelI64Result {
            is_ok: false,
            value: 0,
            error: STATUS_TIMEOUT_CODE,
        };
    };

    match state.outcome {
        TimeoutCancelOutcome::Completed => TimeoutCancelI64Result {
            is_ok: true,
            value: state.completed_value,
            error: 0,
        },
        TimeoutCancelOutcome::TimedOut => TimeoutCancelI64Result {
            is_ok: false,
            value: 0,
            error: STATUS_TIMEOUT_CODE,
        },
        TimeoutCancelOutcome::Pending => TimeoutCancelI64Result {
            is_ok: false,
            value: 0,
            error: STATUS_TIMEOUT_CODE,
        },
    }
}

#[no_mangle]
/// # Safety
///
/// `handle` must be zero or an unconsumed handle returned by
/// [`sengoo_async_timeout_cancel_i64__start`].
pub unsafe extern "C" fn sengoo_async_timeout_cancel_i64__cancel(handle: i64) -> bool {
    let Some(state) = handle_take_box::<TimeoutCancelI64FutureState>(handle) else {
        return false;
    };
    if state.outcome == TimeoutCancelOutcome::Pending {
        let _ = sengoo_async_cancel_dispatch(state.child_kind, state.child_handle);
    }
    true
}

#[no_mangle]
/// # Safety
///
/// `handle` must be zero or an unconsumed handle returned by
/// [`sengoo_async_timeout_cancel_i64__start`].
pub unsafe extern "C" fn sengoo_async_timeout_cancel_i64__drop(handle: i64) {
    let Some(state) = handle_take_box::<TimeoutCancelI64FutureState>(handle) else {
        return;
    };
    if state.outcome == TimeoutCancelOutcome::Pending {
        let _ = sengoo_async_cancel_dispatch(state.child_kind, state.child_handle);
    }
    drop(state);
}
