use std::time::{Duration, Instant};

use super::{
    clear_poll_wakeup_hint, handle_mut, handle_ref, handle_take_box,
    merge_wakeup_hint_with_deadline, record_poll_wakeup_hint, sengoo_async_poll_dispatch,
    take_poll_wakeup_hint,
};

#[derive(Debug)]
pub(super) struct SleepFutureState {
    pub(super) deadline: Instant,
}

struct TimeoutBoolFutureState {
    child_kind: i64,
    child_handle: i64,
    deadline: Instant,
    result: Option<bool>,
}

fn sleep_duration(duration_ms: i64) -> Duration {
    Duration::from_millis(duration_ms.max(0) as u64)
}

#[no_mangle]
pub extern "C" fn sengoo_async_sleep__start(duration_ms: i64) -> i64 {
    let state = SleepFutureState {
        deadline: Instant::now() + sleep_duration(duration_ms),
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
    if Instant::now() >= state.deadline {
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
