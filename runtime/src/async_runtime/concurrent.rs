use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use super::futures::PollLifecycle;
use super::thread_pool::{is_thread_pool_enabled, submit_pool_job};
use super::{handle_mut, handle_ref, handle_take_box, record_poll_wakeup_hint};

pub(crate) const STATUS_INVALID_HANDLE: i64 = 3;

pub(crate) struct PoolJobHandle {
    pub work_fn: extern "C" fn() -> i64,
    pub result: Arc<Mutex<Option<i64>>>,
    pub completed: Arc<AtomicBool>,
    pub canceled: Arc<AtomicBool>,
}

impl PoolJobHandle {
    pub(crate) fn new(work_fn: extern "C" fn() -> i64) -> Self {
        Self {
            work_fn,
            result: Arc::new(Mutex::new(None)),
            completed: Arc::new(AtomicBool::new(false)),
            canceled: Arc::new(AtomicBool::new(false)),
        }
    }

    pub(crate) fn clone_for_queue(&self) -> Self {
        Self {
            work_fn: self.work_fn,
            result: self.result.clone(),
            completed: self.completed.clone(),
            canceled: self.canceled.clone(),
        }
    }
}

struct SpawnBlockingI64State {
    job: PoolJobHandle,
    lifecycle: PollLifecycle,
}

#[no_mangle]
pub extern "C" fn sengoo_async_runtime_enable_thread_pool(worker_count: i64) -> i64 {
    match super::thread_pool::runtime_enable_thread_pool(worker_count) {
        Ok(value) => value,
        Err(code) => -code,
    }
}

#[no_mangle]
pub extern "C" fn sengoo_async_runtime_thread_pool_enabled() -> bool {
    is_thread_pool_enabled()
}

#[no_mangle]
pub extern "C" fn sengoo_async_spawn_blocking_i64__start(work_fn: extern "C" fn() -> i64) -> i64 {
    let Some(job) = submit_pool_job(work_fn) else {
        return 0;
    };
    let state = SpawnBlockingI64State {
        job,
        lifecycle: PollLifecycle::default(),
    };
    Box::into_raw(Box::new(state)) as i64
}

#[no_mangle]
/// # Safety
///
/// `handle` must be zero or a live handle returned by
/// [`sengoo_async_spawn_blocking_i64__start`].
pub unsafe extern "C" fn sengoo_async_spawn_blocking_i64__poll(handle: i64) -> i64 {
    let Some(state) = handle_ref::<SpawnBlockingI64State>(handle) else {
        return 1;
    };
    let guard = match state.lifecycle.enter() {
        Ok(guard) => guard,
        Err(error) => return error,
    };
    if state.job.completed.load(Ordering::Acquire) {
        guard.mark_ready();
        return 1;
    }
    record_poll_wakeup_hint(std::time::Instant::now());
    0
}

#[no_mangle]
/// # Safety
///
/// `handle` must be zero or an unconsumed handle returned by
/// [`sengoo_async_spawn_blocking_i64__start`].
pub unsafe extern "C" fn sengoo_async_spawn_blocking_i64__result(handle: i64) -> i64 {
    let Some(state) = handle_take_box::<SpawnBlockingI64State>(handle) else {
        return 0;
    };
    let value = state
        .job
        .result
        .lock()
        .expect("spawn blocking result mutex poisoned")
        .unwrap_or(0);
    value
}

#[no_mangle]
/// # Safety
///
/// `handle` must be zero or an unconsumed handle returned by
/// [`sengoo_async_spawn_blocking_i64__start`].
pub unsafe extern "C" fn sengoo_async_spawn_blocking_i64__cancel(handle: i64) -> bool {
    let Some(state) = handle_take_box::<SpawnBlockingI64State>(handle) else {
        return false;
    };
    state.job.canceled.store(true, Ordering::Release);
    true
}

#[no_mangle]
/// # Safety
///
/// `handle` must be zero or an unconsumed handle returned by
/// [`sengoo_async_spawn_blocking_i64__start`].
pub unsafe extern "C" fn sengoo_async_spawn_blocking_i64__drop(handle: i64) {
    let Some(state) = handle_take_box::<SpawnBlockingI64State>(handle) else {
        return;
    };
    state.job.canceled.store(true, Ordering::Release);
    drop(state);
}

struct ChannelInner {
    capacity: usize,
    queue: VecDeque<i64>,
    sender_count: usize,
    closed: bool,
}

struct ChannelShared {
    inner: Mutex<ChannelInner>,
}

struct ChannelSender {
    channel: Arc<ChannelShared>,
}

struct ChannelReceiver {
    channel: Arc<ChannelShared>,
}

struct ChannelSendI64State {
    channel: Arc<ChannelShared>,
    value: i64,
    lifecycle: PollLifecycle,
    outcome: Option<Result<(), i64>>,
}

struct ChannelRecvI64State {
    channel: Arc<ChannelShared>,
    lifecycle: PollLifecycle,
    outcome: Option<Result<i64, i64>>,
}

#[no_mangle]
pub extern "C" fn sengoo_async_channel_bounded_i64(capacity: i64) -> i64 {
    if capacity < 1 {
        return 0;
    }
    let channel = Arc::new(ChannelShared {
        inner: Mutex::new(ChannelInner {
            capacity: capacity as usize,
            queue: VecDeque::new(),
            sender_count: 1,
            closed: false,
        }),
    });
    let pair = (
        ChannelSender {
            channel: channel.clone(),
        },
        ChannelReceiver { channel },
    );
    Box::into_raw(Box::new(pair)) as i64
}

#[no_mangle]
/// # Safety
///
/// `pair_handle` must be a live handle returned by
/// [`sengoo_async_channel_bounded_i64`].
pub unsafe extern "C" fn sengoo_async_channel_pair_sender(pair_handle: i64) -> i64 {
    let Some(pair) = handle_ref::<(ChannelSender, ChannelReceiver)>(pair_handle) else {
        return 0;
    };
    let sender = ChannelSender {
        channel: pair.0.channel.clone(),
    };
    Box::into_raw(Box::new(sender)) as i64
}

#[no_mangle]
/// # Safety
///
/// `pair_handle` must be a live handle returned by
/// [`sengoo_async_channel_bounded_i64`].
pub unsafe extern "C" fn sengoo_async_channel_pair_receiver(pair_handle: i64) -> i64 {
    let Some(pair) = handle_ref::<(ChannelSender, ChannelReceiver)>(pair_handle) else {
        return 0;
    };
    let receiver = ChannelReceiver {
        channel: pair.1.channel.clone(),
    };
    Box::into_raw(Box::new(receiver)) as i64
}

#[no_mangle]
/// # Safety
///
/// `pair_handle` must be a live handle returned by
/// [`sengoo_async_channel_bounded_i64`].
pub unsafe extern "C" fn sengoo_async_channel_pair_free(pair_handle: i64) {
    let Some(pair) = handle_take_box::<(ChannelSender, ChannelReceiver)>(pair_handle) else {
        return;
    };
    drop(pair);
}

#[no_mangle]
/// # Safety
///
/// `sender_handle` must be a live handle returned by
/// [`sengoo_async_channel_pair_sender`].
pub unsafe extern "C" fn sengoo_async_channel_sender_clone(sender_handle: i64) -> i64 {
    let Some(sender) = handle_ref::<ChannelSender>(sender_handle) else {
        return 0;
    };
    let cloned = ChannelSender {
        channel: sender.channel.clone(),
    };
    if let Ok(mut inner) = cloned.channel.inner.lock() {
        inner.sender_count += 1;
    }
    Box::into_raw(Box::new(cloned)) as i64
}

#[no_mangle]
/// # Safety
///
/// `sender_handle` must be a live handle returned by
/// [`sengoo_async_channel_pair_sender`] or
/// [`sengoo_async_channel_sender_clone`].
pub unsafe extern "C" fn sengoo_async_channel_sender_close(sender_handle: i64) {
    let Some(sender) = handle_take_box::<ChannelSender>(sender_handle) else {
        return;
    };
    if let Ok(mut inner) = sender.channel.inner.lock() {
        inner.sender_count = inner.sender_count.saturating_sub(1);
        if inner.sender_count == 0 {
            inner.closed = true;
        }
    };
}

#[no_mangle]
/// # Safety
///
/// `sender_handle` must be a live handle returned by
/// [`sengoo_async_channel_pair_sender`] or
/// [`sengoo_async_channel_sender_clone`].
pub unsafe extern "C" fn sengoo_async_channel_sender_drop(sender_handle: i64) {
    sengoo_async_channel_sender_close(sender_handle);
}

#[no_mangle]
pub extern "C" fn sengoo_async_channel_send_i64__start(sender_handle: i64, value: i64) -> i64 {
    let Some(sender) = (unsafe { handle_ref::<ChannelSender>(sender_handle) }) else {
        return 0;
    };
    let state = ChannelSendI64State {
        channel: sender.channel.clone(),
        value,
        lifecycle: PollLifecycle::default(),
        outcome: None,
    };
    Box::into_raw(Box::new(state)) as i64
}

#[no_mangle]
/// # Safety
///
/// `handle` must be a live handle returned by
/// [`sengoo_async_channel_send_i64__start`].
pub unsafe extern "C" fn sengoo_async_channel_send_i64__poll(handle: i64) -> i64 {
    let Some(state) = handle_mut::<ChannelSendI64State>(handle) else {
        return 1;
    };
    let guard = match state.lifecycle.enter() {
        Ok(guard) => guard,
        Err(error) => return error,
    };
    if state.outcome.is_some() {
        guard.mark_ready();
        return 1;
    }

    let mut inner = state
        .channel
        .inner
        .lock()
        .expect("channel inner mutex poisoned");
    if inner.closed {
        state.outcome = Some(Err(STATUS_INVALID_HANDLE));
        guard.mark_ready();
        return 1;
    }
    if inner.queue.len() < inner.capacity {
        inner.queue.push_back(state.value);
        state.outcome = Some(Ok(()));
        guard.mark_ready();
        return 1;
    }
    record_poll_wakeup_hint(std::time::Instant::now());
    0
}

#[repr(C)]
pub struct ChannelSendI64Result {
    pub is_ok: bool,
    pub error: i64,
}

#[no_mangle]
/// # Safety
///
/// `handle` must be an unconsumed handle returned by
/// [`sengoo_async_channel_send_i64__start`].
pub unsafe extern "C" fn sengoo_async_channel_send_i64__result(
    handle: i64,
) -> ChannelSendI64Result {
    let Some(state) = handle_take_box::<ChannelSendI64State>(handle) else {
        return ChannelSendI64Result {
            is_ok: false,
            error: STATUS_INVALID_HANDLE,
        };
    };
    match state.outcome {
        Some(Ok(())) => ChannelSendI64Result {
            is_ok: true,
            error: 0,
        },
        Some(Err(code)) => ChannelSendI64Result {
            is_ok: false,
            error: code,
        },
        None => ChannelSendI64Result {
            is_ok: false,
            error: STATUS_INVALID_HANDLE,
        },
    }
}

#[no_mangle]
/// # Safety
///
/// `handle` must be an unconsumed handle returned by
/// [`sengoo_async_channel_send_i64__start`].
pub unsafe extern "C" fn sengoo_async_channel_send_i64__cancel(handle: i64) -> bool {
    let Some(state) = handle_take_box::<ChannelSendI64State>(handle) else {
        return false;
    };
    drop(state);
    true
}

#[no_mangle]
/// # Safety
///
/// `handle` must be an unconsumed handle returned by
/// [`sengoo_async_channel_send_i64__start`].
pub unsafe extern "C" fn sengoo_async_channel_send_i64__drop(handle: i64) {
    let Some(state) = handle_take_box::<ChannelSendI64State>(handle) else {
        return;
    };
    drop(state);
}

#[no_mangle]
pub extern "C" fn sengoo_async_channel_recv_i64__start(receiver_handle: i64) -> i64 {
    let Some(receiver) = (unsafe { handle_ref::<ChannelReceiver>(receiver_handle) }) else {
        return 0;
    };
    let state = ChannelRecvI64State {
        channel: receiver.channel.clone(),
        lifecycle: PollLifecycle::default(),
        outcome: None,
    };
    Box::into_raw(Box::new(state)) as i64
}

#[no_mangle]
/// # Safety
///
/// `handle` must be a live handle returned by
/// [`sengoo_async_channel_recv_i64__start`].
pub unsafe extern "C" fn sengoo_async_channel_recv_i64__poll(handle: i64) -> i64 {
    let Some(state) = handle_mut::<ChannelRecvI64State>(handle) else {
        return 1;
    };
    let guard = match state.lifecycle.enter() {
        Ok(guard) => guard,
        Err(error) => return error,
    };
    if state.outcome.is_some() {
        guard.mark_ready();
        return 1;
    }

    let mut inner = state
        .channel
        .inner
        .lock()
        .expect("channel inner mutex poisoned");
    if let Some(value) = inner.queue.pop_front() {
        state.outcome = Some(Ok(value));
        guard.mark_ready();
        return 1;
    }
    if inner.closed {
        state.outcome = Some(Err(STATUS_INVALID_HANDLE));
        guard.mark_ready();
        return 1;
    }
    record_poll_wakeup_hint(std::time::Instant::now());
    0
}

#[repr(C)]
pub struct ChannelRecvI64Result {
    pub is_ok: bool,
    pub value: i64,
    pub error: i64,
}

#[no_mangle]
/// # Safety
///
/// `handle` must be an unconsumed handle returned by
/// [`sengoo_async_channel_recv_i64__start`].
pub unsafe extern "C" fn sengoo_async_channel_recv_i64__result(
    handle: i64,
) -> ChannelRecvI64Result {
    let Some(state) = handle_take_box::<ChannelRecvI64State>(handle) else {
        return ChannelRecvI64Result {
            is_ok: false,
            value: 0,
            error: STATUS_INVALID_HANDLE,
        };
    };
    match state.outcome {
        Some(Ok(value)) => ChannelRecvI64Result {
            is_ok: true,
            value,
            error: 0,
        },
        Some(Err(code)) => ChannelRecvI64Result {
            is_ok: false,
            value: 0,
            error: code,
        },
        None => ChannelRecvI64Result {
            is_ok: false,
            value: 0,
            error: STATUS_INVALID_HANDLE,
        },
    }
}

#[no_mangle]
/// # Safety
///
/// `handle` must be an unconsumed handle returned by
/// [`sengoo_async_channel_recv_i64__start`].
pub unsafe extern "C" fn sengoo_async_channel_recv_i64__cancel(handle: i64) -> bool {
    let Some(state) = handle_take_box::<ChannelRecvI64State>(handle) else {
        return false;
    };
    drop(state);
    true
}

#[no_mangle]
/// # Safety
///
/// `handle` must be an unconsumed handle returned by
/// [`sengoo_async_channel_recv_i64__start`].
pub unsafe extern "C" fn sengoo_async_channel_recv_i64__drop(handle: i64) {
    let Some(state) = handle_take_box::<ChannelRecvI64State>(handle) else {
        return;
    };
    drop(state);
}

struct AsyncMutexInner {
    locked: bool,
    value: i64,
    closed: bool,
    waiters: AtomicUsize,
}

struct AsyncMutexState {
    inner: Arc<Mutex<AsyncMutexInner>>,
}

struct MutexLockI64State {
    mutex: Arc<Mutex<AsyncMutexInner>>,
    lifecycle: PollLifecycle,
    outcome: Option<Result<i64, i64>>,
}

#[no_mangle]
pub extern "C" fn sengoo_async_mutex_new_i64(value: i64) -> i64 {
    let state = AsyncMutexState {
        inner: Arc::new(Mutex::new(AsyncMutexInner {
            locked: false,
            value,
            closed: false,
            waiters: AtomicUsize::new(0),
        })),
    };
    Box::into_raw(Box::new(state)) as i64
}

#[no_mangle]
/// # Safety
///
/// `handle` must be a live handle returned by [`sengoo_async_mutex_new_i64`].
pub unsafe extern "C" fn sengoo_async_mutex_close(handle: i64) {
    let Some(state) = handle_ref::<AsyncMutexState>(handle) else {
        return;
    };
    if let Ok(mut inner) = state.inner.lock() {
        inner.closed = true;
    }
}

#[no_mangle]
/// # Safety
///
/// `handle` must be a live handle returned by [`sengoo_async_mutex_new_i64`].
pub unsafe extern "C" fn sengoo_async_mutex_drop(handle: i64) {
    let Some(state) = handle_take_box::<AsyncMutexState>(handle) else {
        return;
    };
    if let Ok(mut inner) = state.inner.lock() {
        inner.closed = true;
    }
    drop(state);
}

#[no_mangle]
pub extern "C" fn sengoo_async_mutex_lock_i64__start(mutex_handle: i64) -> i64 {
    let Some(mutex_state) = (unsafe { handle_ref::<AsyncMutexState>(mutex_handle) }) else {
        return 0;
    };
    let state = MutexLockI64State {
        mutex: mutex_state.inner.clone(),
        lifecycle: PollLifecycle::default(),
        outcome: None,
    };
    Box::into_raw(Box::new(state)) as i64
}

#[no_mangle]
/// # Safety
///
/// `handle` must be a live handle returned by
/// [`sengoo_async_mutex_lock_i64__start`].
pub unsafe extern "C" fn sengoo_async_mutex_lock_i64__poll(handle: i64) -> i64 {
    let Some(state) = handle_mut::<MutexLockI64State>(handle) else {
        return 1;
    };
    let guard = match state.lifecycle.enter() {
        Ok(guard) => guard,
        Err(error) => return error,
    };
    if state.outcome.is_some() {
        guard.mark_ready();
        return 1;
    }

    let mut inner = state.mutex.lock().expect("async mutex inner poisoned");
    if inner.closed {
        state.outcome = Some(Err(STATUS_INVALID_HANDLE));
        guard.mark_ready();
        return 1;
    }
    if !inner.locked {
        inner.locked = true;
        state.outcome = Some(Ok(inner.value));
        guard.mark_ready();
        return 1;
    }
    inner.waiters.fetch_add(1, Ordering::AcqRel);
    record_poll_wakeup_hint(std::time::Instant::now());
    0
}

#[repr(C)]
pub struct MutexLockI64Result {
    pub is_ok: bool,
    pub value: i64,
    pub error: i64,
}

#[no_mangle]
/// # Safety
///
/// `handle` must be an unconsumed handle returned by
/// [`sengoo_async_mutex_lock_i64__start`].
pub unsafe extern "C" fn sengoo_async_mutex_lock_i64__result(handle: i64) -> MutexLockI64Result {
    let Some(state) = handle_take_box::<MutexLockI64State>(handle) else {
        return MutexLockI64Result {
            is_ok: false,
            value: 0,
            error: STATUS_INVALID_HANDLE,
        };
    };
    match state.outcome {
        Some(Ok(value)) => MutexLockI64Result {
            is_ok: true,
            value,
            error: 0,
        },
        Some(Err(code)) => MutexLockI64Result {
            is_ok: false,
            value: 0,
            error: code,
        },
        None => MutexLockI64Result {
            is_ok: false,
            value: 0,
            error: STATUS_INVALID_HANDLE,
        },
    }
}

#[no_mangle]
/// # Safety
///
/// `handle` must be an unconsumed handle returned by
/// [`sengoo_async_mutex_lock_i64__start`].
pub unsafe extern "C" fn sengoo_async_mutex_lock_i64__cancel(handle: i64) -> bool {
    let Some(state) = handle_take_box::<MutexLockI64State>(handle) else {
        return false;
    };
    if let Ok(inner) = state.mutex.lock() {
        let _ = inner.waiters.fetch_sub(1, Ordering::AcqRel);
    }
    drop(state);
    true
}

#[no_mangle]
/// # Safety
///
/// `handle` must be an unconsumed handle returned by
/// [`sengoo_async_mutex_lock_i64__start`].
pub unsafe extern "C" fn sengoo_async_mutex_lock_i64__drop(handle: i64) {
    let Some(state) = handle_take_box::<MutexLockI64State>(handle) else {
        return;
    };
    if let Ok(inner) = state.mutex.lock() {
        let _ = inner.waiters.fetch_sub(1, Ordering::AcqRel);
    }
    drop(state);
}

#[no_mangle]
/// # Safety
///
/// `mutex_handle` must be a live handle returned by [`sengoo_async_mutex_new_i64`].
pub unsafe extern "C" fn sengoo_async_mutex_unlock_i64(mutex_handle: i64, value: i64) -> i64 {
    let Some(state) = handle_ref::<AsyncMutexState>(mutex_handle) else {
        return -STATUS_INVALID_HANDLE;
    };
    let Ok(mut inner) = state.inner.lock() else {
        return -STATUS_INVALID_HANDLE;
    };
    if inner.closed {
        return -STATUS_INVALID_HANDLE;
    }
    inner.value = value;
    inner.locked = false;
    0
}

pub(crate) fn retain_native_bridge_exports_for_linker() {
    macro_rules! export_addr {
        ($symbol:path) => {
            $symbol as *const () as usize
        };
    }

    let _ = [
        export_addr!(sengoo_async_runtime_enable_thread_pool),
        export_addr!(sengoo_async_runtime_thread_pool_enabled),
        export_addr!(sengoo_async_spawn_blocking_i64__start),
        export_addr!(sengoo_async_spawn_blocking_i64__poll),
        export_addr!(sengoo_async_spawn_blocking_i64__result),
        export_addr!(sengoo_async_spawn_blocking_i64__cancel),
        export_addr!(sengoo_async_spawn_blocking_i64__drop),
        export_addr!(sengoo_async_channel_bounded_i64),
        export_addr!(sengoo_async_channel_pair_sender),
        export_addr!(sengoo_async_channel_pair_receiver),
        export_addr!(sengoo_async_channel_pair_free),
        export_addr!(sengoo_async_channel_sender_clone),
        export_addr!(sengoo_async_channel_sender_close),
        export_addr!(sengoo_async_channel_sender_drop),
        export_addr!(sengoo_async_channel_send_i64__start),
        export_addr!(sengoo_async_channel_send_i64__poll),
        export_addr!(sengoo_async_channel_send_i64__result),
        export_addr!(sengoo_async_channel_send_i64__cancel),
        export_addr!(sengoo_async_channel_send_i64__drop),
        export_addr!(sengoo_async_channel_recv_i64__start),
        export_addr!(sengoo_async_channel_recv_i64__poll),
        export_addr!(sengoo_async_channel_recv_i64__result),
        export_addr!(sengoo_async_channel_recv_i64__cancel),
        export_addr!(sengoo_async_channel_recv_i64__drop),
        export_addr!(sengoo_async_mutex_new_i64),
        export_addr!(sengoo_async_mutex_close),
        export_addr!(sengoo_async_mutex_drop),
        export_addr!(sengoo_async_mutex_lock_i64__start),
        export_addr!(sengoo_async_mutex_lock_i64__poll),
        export_addr!(sengoo_async_mutex_lock_i64__result),
        export_addr!(sengoo_async_mutex_lock_i64__cancel),
        export_addr!(sengoo_async_mutex_lock_i64__drop),
        export_addr!(sengoo_async_mutex_unlock_i64),
    ];
}
