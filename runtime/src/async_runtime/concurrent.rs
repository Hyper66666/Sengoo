use std::alloc::{alloc, dealloc, Layout};
use std::collections::{HashMap, VecDeque};
use std::ffi::c_void;
use std::ptr::NonNull;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use super::futures::PollLifecycle;
use super::thread_pool::{is_thread_pool_enabled, submit_pool_job, submit_pool_task};
use super::{handle_mut, handle_ref, handle_take_box, record_poll_wakeup_hint};

pub(crate) const STATUS_INVALID_ARGUMENT: i64 = 2;
pub(crate) const STATUS_INVALID_HANDLE: i64 = 3;
pub(crate) const STATUS_LOCK_UNAVAILABLE: i64 = 11;
pub const SENGOO_COLLECTIONS_ABI_VERSION: u32 = 1;

type PoolWork = Box<dyn FnOnce() -> i64 + Send + 'static>;
type SharedPoolWork = Arc<Mutex<Option<PoolWork>>>;

pub(crate) struct PoolJobHandle {
    work: SharedPoolWork,
    pub result: Arc<Mutex<Option<i64>>>,
    pub completed: Arc<AtomicBool>,
    pub canceled: Arc<AtomicBool>,
}

impl PoolJobHandle {
    pub(crate) fn new(work_fn: extern "C" fn() -> i64) -> Self {
        Self::new_task(Box::new(move || work_fn()))
    }

    pub(crate) fn new_task(work: PoolWork) -> Self {
        Self {
            work: Arc::new(Mutex::new(Some(work))),
            result: Arc::new(Mutex::new(None)),
            completed: Arc::new(AtomicBool::new(false)),
            canceled: Arc::new(AtomicBool::new(false)),
        }
    }

    pub(crate) fn clone_for_queue(&self) -> Self {
        Self {
            work: self.work.clone(),
            result: self.result.clone(),
            completed: self.completed.clone(),
            canceled: self.canceled.clone(),
        }
    }

    pub(crate) fn execute(&self) -> Option<i64> {
        let work = self.work.lock().ok()?.take()?;
        Some(work())
    }
}

struct SpawnBlockingI64State {
    job: PoolJobHandle,
    lifecycle: PollLifecycle,
}

pub type SengooMoveFn = unsafe extern "C" fn(destination: *mut c_void, source: *mut c_void);
pub type SengooDropFn = unsafe extern "C" fn(value: *mut c_void);
pub type SengooCloneFn =
    unsafe extern "C" fn(destination: *mut c_void, source: *const c_void) -> i32;
pub type SengooHashFn = unsafe extern "C" fn(value: *const c_void) -> u64;
pub type SengooEqFn = unsafe extern "C" fn(left: *const c_void, right: *const c_void) -> i64;
pub type SengooCompareFn = unsafe extern "C" fn(left: *const c_void, right: *const c_void) -> i64;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct SengooTypeDescriptor {
    pub abi_version: u32,
    pub flags: u32,
    pub size: usize,
    pub align: usize,
    pub move_value: Option<SengooMoveFn>,
    pub drop_value: Option<SengooDropFn>,
    pub clone_value: Option<SengooCloneFn>,
    pub hash_value: Option<SengooHashFn>,
    pub eq_value: Option<SengooEqFn>,
    pub compare_value: Option<SengooCompareFn>,
}

fn validate_descriptor(descriptor: &SengooTypeDescriptor) -> Option<Layout> {
    if descriptor.abi_version != SENGOO_COLLECTIONS_ABI_VERSION
        || descriptor.size == 0
        || descriptor.align == 0
        || !descriptor.align.is_power_of_two()
        || descriptor.move_value.is_none()
        || descriptor.drop_value.is_none()
    {
        return None;
    }
    Layout::from_size_align(descriptor.size, descriptor.align).ok()
}

fn descriptor_from_parts(
    size: i64,
    align: i64,
    move_value: Option<SengooMoveFn>,
    drop_value: Option<SengooDropFn>,
) -> Option<SengooTypeDescriptor> {
    if size <= 0 || align <= 0 {
        return None;
    }
    let move_value = move_value?;
    let drop_value = drop_value?;
    Some(SengooTypeDescriptor {
        abi_version: SENGOO_COLLECTIONS_ABI_VERSION,
        flags: 0,
        size: size as usize,
        align: align as usize,
        move_value: Some(move_value),
        drop_value: Some(drop_value),
        clone_value: None,
        hash_value: None,
        eq_value: None,
        compare_value: None,
    })
}

struct OwnedDescriptorValue {
    descriptor: SengooTypeDescriptor,
    layout: Layout,
    ptr: NonNull<u8>,
}

impl OwnedDescriptorValue {
    unsafe fn new(descriptor: &SengooTypeDescriptor, value: *mut c_void) -> Option<Self> {
        let layout = validate_descriptor(descriptor)?;
        if value.is_null() {
            return None;
        }
        let raw = alloc(layout);
        let ptr = NonNull::new(raw)?;
        descriptor
            .move_value
            .expect("validated descriptor keeps move")(ptr.as_ptr().cast::<c_void>(), value);
        Some(Self {
            descriptor: *descriptor,
            layout,
            ptr,
        })
    }

    fn as_ptr(&self) -> *mut c_void {
        self.ptr.as_ptr().cast::<c_void>()
    }
}

impl Drop for OwnedDescriptorValue {
    fn drop(&mut self) {
        unsafe {
            self.descriptor
                .drop_value
                .expect("validated descriptor keeps drop")(self.as_ptr());
            dealloc(self.ptr.as_ptr(), self.layout);
        }
    }
}

unsafe impl Send for OwnedDescriptorValue {}
unsafe impl Sync for OwnedDescriptorValue {}

struct SengooArcInner {
    payload: OwnedDescriptorValue,
}

struct SengooArcHandle {
    inner: Arc<SengooArcInner>,
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

#[repr(C)]
struct SharedCounterMutexPayload {
    handle: i64,
    marker: i64,
}

unsafe extern "C" fn shared_counter_mutex_move(destination: *mut c_void, source: *mut c_void) {
    std::ptr::copy_nonoverlapping(source.cast::<u8>(), destination.cast::<u8>(), 16);
    std::ptr::write_bytes(source.cast::<u8>(), 0, 16);
}

unsafe extern "C" fn shared_counter_mutex_drop(value: *mut c_void) {
    if value.is_null() {
        return;
    }
    let payload = value.cast::<SharedCounterMutexPayload>();
    let handle = unsafe { (*payload).handle };
    if handle != 0 {
        unsafe { sengoo_async_mutex_drop(handle) };
    }
}

fn shared_counter_mutex_descriptor() -> SengooTypeDescriptor {
    SengooTypeDescriptor {
        abi_version: SENGOO_COLLECTIONS_ABI_VERSION,
        flags: 0,
        size: std::mem::size_of::<SharedCounterMutexPayload>(),
        align: std::mem::align_of::<SharedCounterMutexPayload>(),
        move_value: Some(shared_counter_mutex_move),
        drop_value: Some(shared_counter_mutex_drop),
        clone_value: None,
        hash_value: None,
        eq_value: None,
        compare_value: None,
    }
}

fn shared_counter_mutex_handle(handle: i64) -> Option<i64> {
    let borrowed = unsafe { sengoo_arc_borrow_ptr(handle) };
    let payload = NonNull::new(borrowed.cast::<SharedCounterMutexPayload>())?;
    Some(unsafe { payload.as_ref().handle })
}

#[no_mangle]
/// # Safety
///
/// `descriptor` must describe the owned payload pointed to by `value`.
pub unsafe extern "C" fn sengoo_arc_new(
    descriptor: *const SengooTypeDescriptor,
    value: *mut c_void,
) -> i64 {
    let Some(descriptor) = descriptor.as_ref() else {
        return 0;
    };
    let Some(payload) = OwnedDescriptorValue::new(descriptor, value) else {
        return 0;
    };
    Box::into_raw(Box::new(SengooArcHandle {
        inner: Arc::new(SengooArcInner { payload }),
    })) as i64
}

#[no_mangle]
/// # Safety
///
/// `value` must point to an owned payload compatible with the supplied parts.
pub unsafe extern "C" fn sengoo_arc_new_parts(
    value: *mut c_void,
    size: i64,
    align: i64,
    move_value: Option<SengooMoveFn>,
    drop_value: Option<SengooDropFn>,
) -> i64 {
    let Some(descriptor) = descriptor_from_parts(size, align, move_value, drop_value) else {
        return 0;
    };
    unsafe { sengoo_arc_new(&descriptor, value) }
}

#[no_mangle]
/// # Safety
///
/// `handle` must be a live generic Arc handle.
pub unsafe extern "C" fn sengoo_arc_clone(handle: i64) -> i64 {
    let Some(state) = handle_ref::<SengooArcHandle>(handle) else {
        return 0;
    };
    Box::into_raw(Box::new(SengooArcHandle {
        inner: state.inner.clone(),
    })) as i64
}

#[no_mangle]
/// # Safety
///
/// `handle` must be a live generic Arc handle.
pub unsafe extern "C" fn sengoo_arc_strong_count(handle: i64) -> i64 {
    let Some(state) = handle_ref::<SengooArcHandle>(handle) else {
        return 0;
    };
    Arc::strong_count(&state.inner) as i64
}

#[no_mangle]
/// # Safety
///
/// `handle` must be a live generic Arc handle.
pub unsafe extern "C" fn sengoo_arc_borrow_ptr(handle: i64) -> *mut c_void {
    let Some(state) = handle_ref::<SengooArcHandle>(handle) else {
        return std::ptr::null_mut();
    };
    state.inner.payload.as_ptr()
}

#[no_mangle]
/// # Safety
///
/// `handle` must be a live, unconsumed generic Arc handle.
pub unsafe extern "C" fn sengoo_arc_drop(handle: i64) {
    let Some(state) = handle_take_box::<SengooArcHandle>(handle) else {
        return;
    };
    drop(state);
}

#[no_mangle]
pub extern "C" fn sengoo_async_shared_counter_new_i64(value: i64) -> i64 {
    let mutex = sengoo_async_mutex_new_i64(value);
    if mutex == 0 {
        return 0;
    }
    let mut payload = SharedCounterMutexPayload {
        handle: mutex,
        marker: 0,
    };
    let descriptor = shared_counter_mutex_descriptor();
    unsafe {
        sengoo_arc_new(
            &descriptor,
            (&mut payload as *mut SharedCounterMutexPayload).cast(),
        )
    }
}

#[no_mangle]
/// # Safety
///
/// `handle` must be a live shared-counter handle.
pub unsafe extern "C" fn sengoo_async_shared_counter_clone_i64(handle: i64) -> i64 {
    unsafe { sengoo_arc_clone(handle) }
}

#[no_mangle]
/// # Safety
///
/// `handle` must be a live shared-counter handle.
pub unsafe extern "C" fn sengoo_async_shared_counter_get_i64(handle: i64) -> i64 {
    let Some(mutex_handle) = shared_counter_mutex_handle(handle) else {
        return 0;
    };
    read_i64_from_generic_mutex(mutex_handle).unwrap_or(0)
}

#[no_mangle]
/// # Safety
///
/// `handle` must be a live, unconsumed shared-counter handle.
pub unsafe extern "C" fn sengoo_async_shared_counter_drop(handle: i64) {
    unsafe { sengoo_arc_drop(handle) };
}

#[no_mangle]
/// Submits repeated additions against an atomically shared mutex payload.
///
/// # Safety
///
/// `counter_handle` must be a live shared-counter handle. The runtime clones
/// the backing `Arc`, so the source handle may be dropped after this returns.
pub unsafe extern "C" fn sengoo_async_shared_counter_spawn_add_i64(
    counter_handle: i64,
    delta: i64,
    repetitions: i64,
) -> i64 {
    if repetitions < 0 {
        return 0;
    }
    let Some(counter) = handle_ref::<SengooArcHandle>(counter_handle) else {
        return 0;
    };
    let Some(mutex_handle) = shared_counter_mutex_handle(counter_handle) else {
        return 0;
    };
    let Some(mutex) = clone_generic_mutex_inner(mutex_handle) else {
        return 0;
    };
    let owned_arc = counter.inner.clone();
    let Some(job) = submit_pool_task(move || {
        let _owned_arc = owned_arc;
        for _ in 0..repetitions {
            let counter = mutex.lock().expect("shared counter mutex poisoned");
            if counter.closed {
                return 0;
            }
            let Some(ptr) = counter.payload.as_ptr().cast::<i64>().as_mut() else {
                return 0;
            };
            *ptr = ptr.wrapping_add(delta);
        }
        mutex
            .lock()
            .ok()
            .and_then(|counter| counter.payload.as_ptr().cast::<i64>().as_ref().copied())
            .unwrap_or(0)
    }) else {
        return 0;
    };
    Box::into_raw(Box::new(SpawnBlockingI64State {
        job,
        lifecycle: PollLifecycle::default(),
    })) as i64
}

#[no_mangle]
/// Blocks until the submitted shared-counter job completes and returns that
/// worker's final observed value. The handle remains live until job drop.
///
/// # Safety
///
/// `handle` must be a live shared-counter job handle.
pub unsafe extern "C" fn sengoo_async_shared_counter_join_i64(handle: i64) -> i64 {
    let Some(state) = handle_ref::<SpawnBlockingI64State>(handle) else {
        return 0;
    };
    while !state.job.completed.load(Ordering::Acquire) {
        std::thread::yield_now();
    }
    state
        .job
        .result
        .lock()
        .ok()
        .and_then(|result| *result)
        .unwrap_or(0)
}

#[no_mangle]
/// # Safety
///
/// `handle` must be a live, unconsumed shared-counter job handle.
pub unsafe extern "C" fn sengoo_async_shared_counter_job_drop(handle: i64) {
    sengoo_async_spawn_blocking_i64__drop(handle);
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
    closed: bool,
    waiters: usize,
    payload: OwnedDescriptorValue,
}

struct AsyncMutexState {
    inner: Arc<Mutex<AsyncMutexInner>>,
}

struct MutexLockState {
    mutex: Arc<Mutex<AsyncMutexInner>>,
    lifecycle: PollLifecycle,
    outcome: Option<i64>,
    registered_waiter: bool,
}

struct MutexLockI64State {
    mutex: Arc<Mutex<AsyncMutexInner>>,
    lifecycle: PollLifecycle,
    outcome: Option<Result<i64, i64>>,
    registered_waiter: bool,
}

unsafe extern "C" fn sengoo_scalar_move_i64(destination: *mut c_void, source: *mut c_void) {
    std::ptr::copy_nonoverlapping(source.cast::<u8>(), destination.cast::<u8>(), 8);
    std::ptr::write_bytes(source.cast::<u8>(), 0, 8);
}

unsafe extern "C" fn sengoo_scalar_drop_i64(_value: *mut c_void) {}

fn scalar_i64_descriptor() -> SengooTypeDescriptor {
    SengooTypeDescriptor {
        abi_version: SENGOO_COLLECTIONS_ABI_VERSION,
        flags: 0,
        size: std::mem::size_of::<i64>(),
        align: std::mem::align_of::<i64>(),
        move_value: Some(sengoo_scalar_move_i64),
        drop_value: Some(sengoo_scalar_drop_i64),
        clone_value: None,
        hash_value: None,
        eq_value: None,
        compare_value: None,
    }
}

fn clone_generic_mutex_inner(handle: i64) -> Option<Arc<Mutex<AsyncMutexInner>>> {
    let state = unsafe { handle_ref::<AsyncMutexState>(handle) }?;
    Some(state.inner.clone())
}

fn read_i64_from_generic_mutex(handle: i64) -> Option<i64> {
    let state = unsafe { handle_ref::<AsyncMutexState>(handle) }?;
    let inner = state.inner.lock().ok()?;
    if inner.closed {
        return None;
    }
    unsafe { inner.payload.as_ptr().cast::<i64>().as_ref().copied() }
}

#[no_mangle]
/// # Safety
///
/// `descriptor` must describe the owned payload pointed to by `value`.
pub unsafe extern "C" fn sengoo_async_mutex_new(
    descriptor: *const SengooTypeDescriptor,
    value: *mut c_void,
) -> i64 {
    let Some(descriptor) = descriptor.as_ref() else {
        return 0;
    };
    let Some(payload) = OwnedDescriptorValue::new(descriptor, value) else {
        return 0;
    };
    let state = AsyncMutexState {
        inner: Arc::new(Mutex::new(AsyncMutexInner {
            locked: false,
            closed: false,
            waiters: 0,
            payload,
        })),
    };
    Box::into_raw(Box::new(state)) as i64
}

#[no_mangle]
/// # Safety
///
/// `value` must point to an owned payload compatible with the supplied parts.
pub unsafe extern "C" fn sengoo_async_mutex_new_parts(
    value: *mut c_void,
    size: i64,
    align: i64,
    move_value: Option<SengooMoveFn>,
    drop_value: Option<SengooDropFn>,
) -> i64 {
    let Some(descriptor) = descriptor_from_parts(size, align, move_value, drop_value) else {
        return 0;
    };
    unsafe { sengoo_async_mutex_new(&descriptor, value) }
}

#[no_mangle]
pub extern "C" fn sengoo_async_mutex_new_i64(value: i64) -> i64 {
    let descriptor = scalar_i64_descriptor();
    let mut slot = value;
    unsafe { sengoo_async_mutex_new(&descriptor, (&mut slot as *mut i64).cast::<c_void>()) }
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
        registered_waiter: false,
    };
    Box::into_raw(Box::new(state)) as i64
}

#[no_mangle]
pub extern "C" fn sengoo_async_mutex_lock__start(mutex_handle: i64) -> i64 {
    let Some(mutex_state) = (unsafe { handle_ref::<AsyncMutexState>(mutex_handle) }) else {
        return 0;
    };
    let state = MutexLockState {
        mutex: mutex_state.inner.clone(),
        lifecycle: PollLifecycle::default(),
        outcome: None,
        registered_waiter: false,
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
        if state.registered_waiter {
            inner.waiters = inner.waiters.saturating_sub(1);
            state.registered_waiter = false;
        }
        state.outcome = Some(Ok(inner
            .payload
            .as_ptr()
            .cast::<i64>()
            .as_ref()
            .copied()
            .unwrap_or(0)));
        guard.mark_ready();
        return 1;
    }
    if !state.registered_waiter {
        inner.waiters += 1;
        state.registered_waiter = true;
    }
    record_poll_wakeup_hint(std::time::Instant::now());
    0
}

#[no_mangle]
/// # Safety
///
/// `handle` must be a live handle returned by
/// [`sengoo_async_mutex_lock__start`].
pub unsafe extern "C" fn sengoo_async_mutex_lock__poll(handle: i64) -> i64 {
    let Some(state) = handle_mut::<MutexLockState>(handle) else {
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
        state.outcome = Some(STATUS_INVALID_HANDLE);
        guard.mark_ready();
        return 1;
    }
    if !inner.locked {
        inner.locked = true;
        if state.registered_waiter {
            inner.waiters = inner.waiters.saturating_sub(1);
            state.registered_waiter = false;
        }
        state.outcome = Some(0);
        guard.mark_ready();
        return 1;
    }
    if !state.registered_waiter {
        inner.waiters += 1;
        state.registered_waiter = true;
    }
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
    if state.registered_waiter {
        if let Ok(mut inner) = state.mutex.lock() {
            inner.waiters = inner.waiters.saturating_sub(1);
        }
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
    if state.registered_waiter {
        if let Ok(mut inner) = state.mutex.lock() {
            inner.waiters = inner.waiters.saturating_sub(1);
        }
    }
    drop(state);
}

#[no_mangle]
/// # Safety
///
/// `handle` must be an unconsumed handle returned by
/// [`sengoo_async_mutex_lock__start`].
pub unsafe extern "C" fn sengoo_async_mutex_lock__result(handle: i64) -> i64 {
    let Some(state) = handle_take_box::<MutexLockState>(handle) else {
        return STATUS_INVALID_HANDLE;
    };
    state.outcome.unwrap_or(STATUS_INVALID_HANDLE)
}

#[no_mangle]
/// # Safety
///
/// `handle` must be an unconsumed handle returned by
/// [`sengoo_async_mutex_lock__start`].
pub unsafe extern "C" fn sengoo_async_mutex_lock__cancel(handle: i64) -> bool {
    let Some(state) = handle_take_box::<MutexLockState>(handle) else {
        return false;
    };
    if state.registered_waiter {
        if let Ok(mut inner) = state.mutex.lock() {
            inner.waiters = inner.waiters.saturating_sub(1);
        }
    }
    drop(state);
    true
}

#[no_mangle]
/// # Safety
///
/// `handle` must be an unconsumed handle returned by
/// [`sengoo_async_mutex_lock__start`].
pub unsafe extern "C" fn sengoo_async_mutex_lock__drop(handle: i64) {
    let Some(state) = handle_take_box::<MutexLockState>(handle) else {
        return;
    };
    if state.registered_waiter {
        if let Ok(mut inner) = state.mutex.lock() {
            inner.waiters = inner.waiters.saturating_sub(1);
        }
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
    if !inner.locked {
        return -STATUS_INVALID_HANDLE;
    }
    let Some(slot) = inner.payload.as_ptr().cast::<i64>().as_mut() else {
        return -STATUS_INVALID_HANDLE;
    };
    *slot = value;
    inner.locked = false;
    0
}

#[no_mangle]
/// # Safety
///
/// `mutex_handle` must identify a live, currently locked async mutex.
pub unsafe extern "C" fn sengoo_async_mutex_guard_get_i64(mutex_handle: i64) -> i64 {
    let Some(state) = handle_ref::<AsyncMutexState>(mutex_handle) else {
        return 0;
    };
    let Ok(inner) = state.inner.lock() else {
        return 0;
    };
    if inner.closed || !inner.locked {
        return 0;
    }
    inner
        .payload
        .as_ptr()
        .cast::<i64>()
        .as_ref()
        .copied()
        .unwrap_or(0)
}

#[no_mangle]
/// # Safety
///
/// `mutex_handle` must identify a live, currently locked async mutex.
pub unsafe extern "C" fn sengoo_async_mutex_guard_set_i64(mutex_handle: i64, value: i64) -> i64 {
    let Some(state) = handle_ref::<AsyncMutexState>(mutex_handle) else {
        return -STATUS_INVALID_HANDLE;
    };
    let Ok(inner) = state.inner.lock() else {
        return -STATUS_INVALID_HANDLE;
    };
    if inner.closed || !inner.locked {
        return -STATUS_INVALID_HANDLE;
    }
    let Some(slot) = inner.payload.as_ptr().cast::<i64>().as_mut() else {
        return -STATUS_INVALID_HANDLE;
    };
    *slot = value;
    0
}

#[no_mangle]
/// # Safety
///
/// `mutex_handle` must identify a live, currently locked async mutex.
pub unsafe extern "C" fn sengoo_async_mutex_guard_get(mutex_handle: i64) -> *mut c_void {
    let Some(state) = handle_ref::<AsyncMutexState>(mutex_handle) else {
        return std::ptr::null_mut();
    };
    let Ok(inner) = state.inner.lock() else {
        return std::ptr::null_mut();
    };
    if inner.closed || !inner.locked {
        return std::ptr::null_mut();
    }
    inner.payload.as_ptr()
}

#[no_mangle]
/// Copies a `Copy` payload into caller-owned initialized storage.
///
/// # Safety
///
/// `output` must point to at least `size` writable bytes. On failure the
/// output storage is left unchanged.
pub unsafe extern "C" fn sengoo_async_mutex_guard_copy_into(
    mutex_handle: i64,
    output: *mut c_void,
    size: i64,
) -> i64 {
    if output.is_null() || size <= 0 {
        return -STATUS_INVALID_ARGUMENT;
    }
    let Some(state) = handle_ref::<AsyncMutexState>(mutex_handle) else {
        return -STATUS_INVALID_HANDLE;
    };
    let Ok(inner) = state.inner.lock() else {
        return -STATUS_INVALID_HANDLE;
    };
    if inner.closed || !inner.locked {
        return -STATUS_INVALID_HANDLE;
    }
    if size as usize != inner.payload.descriptor.size {
        return -STATUS_INVALID_ARGUMENT;
    }
    std::ptr::copy_nonoverlapping(
        inner.payload.as_ptr().cast::<u8>(),
        output.cast::<u8>(),
        size as usize,
    );
    0
}

#[no_mangle]
/// # Safety
///
/// `mutex_handle` must identify a live, currently locked async mutex and
/// `value` must point to an owned payload of the mutex element type. When
/// `value` is non-null and `drop_value` is present, this function always
/// consumes the payload: successful replacement moves it into the mutex and
/// every failure path drops it through `drop_value`.
pub unsafe extern "C" fn sengoo_async_mutex_guard_set(
    mutex_handle: i64,
    value: *mut c_void,
    drop_value: Option<SengooDropFn>,
) -> i64 {
    if value.is_null() {
        return -STATUS_INVALID_ARGUMENT;
    }
    let Some(drop_value) = drop_value else {
        return -STATUS_INVALID_ARGUMENT;
    };
    let Some(state) = handle_ref::<AsyncMutexState>(mutex_handle) else {
        drop_value(value);
        return -STATUS_INVALID_HANDLE;
    };
    let Ok(inner) = state.inner.lock() else {
        drop_value(value);
        return -STATUS_INVALID_HANDLE;
    };
    if inner.closed || !inner.locked {
        drop_value(value);
        return -STATUS_INVALID_HANDLE;
    }
    inner
        .payload
        .descriptor
        .drop_value
        .expect("validated descriptor keeps drop")(inner.payload.as_ptr());
    inner
        .payload
        .descriptor
        .move_value
        .expect("validated descriptor keeps move")(inner.payload.as_ptr(), value);
    0
}

#[no_mangle]
/// # Safety
///
/// `mutex_handle` must identify a live, currently locked async mutex.
pub unsafe extern "C" fn sengoo_async_mutex_guard_unlock_i64(mutex_handle: i64) -> i64 {
    let Some(state) = handle_ref::<AsyncMutexState>(mutex_handle) else {
        return -STATUS_INVALID_HANDLE;
    };
    let Ok(mut inner) = state.inner.lock() else {
        return -STATUS_INVALID_HANDLE;
    };
    if inner.closed || !inner.locked {
        return -STATUS_INVALID_HANDLE;
    }
    inner.locked = false;
    0
}

#[no_mangle]
/// # Safety
///
/// `mutex_handle` must identify a live, currently locked async mutex.
pub unsafe extern "C" fn sengoo_async_mutex_guard_unlock(mutex_handle: i64) -> i64 {
    let Some(state) = handle_ref::<AsyncMutexState>(mutex_handle) else {
        return -STATUS_INVALID_HANDLE;
    };
    let Ok(mut inner) = state.inner.lock() else {
        return -STATUS_INVALID_HANDLE;
    };
    if inner.closed || !inner.locked {
        return -STATUS_INVALID_HANDLE;
    }
    inner.locked = false;
    0
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum RwLockGuardKind {
    Read,
    Write,
}

struct AsyncRwLockInner {
    readers: usize,
    writer: bool,
    value: i64,
    closed: bool,
    next_guard_id: i64,
    guards: HashMap<i64, RwLockGuardKind>,
}

struct AsyncRwLockState {
    inner: Arc<Mutex<AsyncRwLockInner>>,
}

fn allocate_rwlock_guard_id(inner: &mut AsyncRwLockInner) -> i64 {
    loop {
        let candidate = inner.next_guard_id.max(1);
        inner.next_guard_id = if candidate == i64::MAX {
            1
        } else {
            candidate + 1
        };
        if !inner.guards.contains_key(&candidate) {
            return candidate;
        }
    }
}

#[no_mangle]
pub extern "C" fn sengoo_async_rwlock_new_i64(value: i64) -> i64 {
    let state = AsyncRwLockState {
        inner: Arc::new(Mutex::new(AsyncRwLockInner {
            readers: 0,
            writer: false,
            value,
            closed: false,
            next_guard_id: 1,
            guards: HashMap::new(),
        })),
    };
    Box::into_raw(Box::new(state)) as i64
}

#[no_mangle]
/// # Safety
///
/// `handle` must be a live handle returned by [`sengoo_async_rwlock_new_i64`].
pub unsafe extern "C" fn sengoo_async_rwlock_close(handle: i64) {
    let Some(state) = handle_ref::<AsyncRwLockState>(handle) else {
        return;
    };
    if let Ok(mut inner) = state.inner.lock() {
        inner.closed = true;
    }
}

#[no_mangle]
/// # Safety
///
/// `handle` must be a live handle returned by [`sengoo_async_rwlock_new_i64`]
/// and no guard may outlive this call.
pub unsafe extern "C" fn sengoo_async_rwlock_drop(handle: i64) {
    let Some(state) = handle_take_box::<AsyncRwLockState>(handle) else {
        return;
    };
    if let Ok(mut inner) = state.inner.lock() {
        inner.closed = true;
    }
    drop(state);
}

#[no_mangle]
/// Attempts to acquire a scalar read guard without blocking.
///
/// Returns a positive guard token on success, `-STATUS_LOCK_UNAVAILABLE` when a
/// writer owns the lock, or `-STATUS_INVALID_HANDLE` for a closed/invalid lock.
///
/// # Safety
///
/// `lock_handle` must be a live handle returned by
/// [`sengoo_async_rwlock_new_i64`].
pub unsafe extern "C" fn sengoo_async_rwlock_try_read_i64(lock_handle: i64) -> i64 {
    let Some(state) = handle_ref::<AsyncRwLockState>(lock_handle) else {
        return -STATUS_INVALID_HANDLE;
    };
    let Ok(mut inner) = state.inner.lock() else {
        return -STATUS_INVALID_HANDLE;
    };
    if inner.closed {
        return -STATUS_INVALID_HANDLE;
    }
    if inner.writer {
        return -STATUS_LOCK_UNAVAILABLE;
    }
    let guard_id = allocate_rwlock_guard_id(&mut inner);
    inner.readers += 1;
    inner.guards.insert(guard_id, RwLockGuardKind::Read);
    guard_id
}

#[no_mangle]
/// Attempts to acquire a scalar write guard without blocking.
///
/// Returns a positive guard token on success, `-STATUS_LOCK_UNAVAILABLE` while
/// any reader or writer owns the lock, or `-STATUS_INVALID_HANDLE` for a
/// closed/invalid lock.
///
/// # Safety
///
/// `lock_handle` must be a live handle returned by
/// [`sengoo_async_rwlock_new_i64`].
pub unsafe extern "C" fn sengoo_async_rwlock_try_write_i64(lock_handle: i64) -> i64 {
    let Some(state) = handle_ref::<AsyncRwLockState>(lock_handle) else {
        return -STATUS_INVALID_HANDLE;
    };
    let Ok(mut inner) = state.inner.lock() else {
        return -STATUS_INVALID_HANDLE;
    };
    if inner.closed {
        return -STATUS_INVALID_HANDLE;
    }
    if inner.writer || inner.readers != 0 {
        return -STATUS_LOCK_UNAVAILABLE;
    }
    let guard_id = allocate_rwlock_guard_id(&mut inner);
    inner.writer = true;
    inner.guards.insert(guard_id, RwLockGuardKind::Write);
    guard_id
}

fn rwlock_guard_value(
    state: &AsyncRwLockState,
    guard_id: i64,
    expected: RwLockGuardKind,
) -> Option<i64> {
    let inner = state.inner.lock().ok()?;
    (inner.guards.get(&guard_id) == Some(&expected)).then_some(inner.value)
}

fn rwlock_guard_unlock(state: &AsyncRwLockState, guard_id: i64, expected: RwLockGuardKind) -> i64 {
    let Ok(mut inner) = state.inner.lock() else {
        return -STATUS_INVALID_HANDLE;
    };
    if inner.guards.get(&guard_id) != Some(&expected) {
        return -STATUS_INVALID_HANDLE;
    }
    inner.guards.remove(&guard_id);
    match expected {
        RwLockGuardKind::Read => inner.readers = inner.readers.saturating_sub(1),
        RwLockGuardKind::Write => inner.writer = false,
    }
    0
}

#[no_mangle]
/// # Safety
///
/// `lock_handle` must identify a live rwlock and `guard_id` must identify one
/// of its active read guards.
pub unsafe extern "C" fn sengoo_async_rwlock_read_guard_get_i64(
    lock_handle: i64,
    guard_id: i64,
) -> i64 {
    let Some(state) = handle_ref::<AsyncRwLockState>(lock_handle) else {
        return 0;
    };
    rwlock_guard_value(state, guard_id, RwLockGuardKind::Read).unwrap_or(0)
}

#[no_mangle]
/// # Safety
///
/// `lock_handle` must identify a live rwlock and `guard_id` must identify one
/// of its active write guards.
pub unsafe extern "C" fn sengoo_async_rwlock_write_guard_get_i64(
    lock_handle: i64,
    guard_id: i64,
) -> i64 {
    let Some(state) = handle_ref::<AsyncRwLockState>(lock_handle) else {
        return 0;
    };
    rwlock_guard_value(state, guard_id, RwLockGuardKind::Write).unwrap_or(0)
}

#[no_mangle]
/// # Safety
///
/// `lock_handle` must identify a live rwlock and `guard_id` must identify one
/// of its active write guards.
pub unsafe extern "C" fn sengoo_async_rwlock_write_guard_set_i64(
    lock_handle: i64,
    guard_id: i64,
    value: i64,
) -> i64 {
    let Some(state) = handle_ref::<AsyncRwLockState>(lock_handle) else {
        return -STATUS_INVALID_HANDLE;
    };
    let Ok(mut inner) = state.inner.lock() else {
        return -STATUS_INVALID_HANDLE;
    };
    if inner.guards.get(&guard_id) != Some(&RwLockGuardKind::Write) {
        return -STATUS_INVALID_HANDLE;
    }
    inner.value = value;
    0
}

#[no_mangle]
/// # Safety
///
/// `lock_handle` must identify a live rwlock. Releasing an already released or
/// mismatched guard token safely returns `-STATUS_INVALID_HANDLE`.
pub unsafe extern "C" fn sengoo_async_rwlock_read_guard_unlock_i64(
    lock_handle: i64,
    guard_id: i64,
) -> i64 {
    let Some(state) = handle_ref::<AsyncRwLockState>(lock_handle) else {
        return -STATUS_INVALID_HANDLE;
    };
    rwlock_guard_unlock(state, guard_id, RwLockGuardKind::Read)
}

#[no_mangle]
/// # Safety
///
/// `lock_handle` must identify a live rwlock. Releasing an already released or
/// mismatched guard token safely returns `-STATUS_INVALID_HANDLE`.
pub unsafe extern "C" fn sengoo_async_rwlock_write_guard_unlock_i64(
    lock_handle: i64,
    guard_id: i64,
) -> i64 {
    let Some(state) = handle_ref::<AsyncRwLockState>(lock_handle) else {
        return -STATUS_INVALID_HANDLE;
    };
    rwlock_guard_unlock(state, guard_id, RwLockGuardKind::Write)
}

pub(crate) fn retain_native_bridge_exports_for_linker() {
    macro_rules! export_addr {
        ($symbol:path) => {
            $symbol as *const () as usize
        };
    }

    let _ = [
        export_addr!(sengoo_arc_new),
        export_addr!(sengoo_arc_new_parts),
        export_addr!(sengoo_arc_clone),
        export_addr!(sengoo_arc_strong_count),
        export_addr!(sengoo_arc_borrow_ptr),
        export_addr!(sengoo_arc_drop),
        export_addr!(sengoo_async_runtime_enable_thread_pool),
        export_addr!(sengoo_async_runtime_thread_pool_enabled),
        export_addr!(sengoo_async_spawn_blocking_i64__start),
        export_addr!(sengoo_async_spawn_blocking_i64__poll),
        export_addr!(sengoo_async_spawn_blocking_i64__result),
        export_addr!(sengoo_async_spawn_blocking_i64__cancel),
        export_addr!(sengoo_async_spawn_blocking_i64__drop),
        export_addr!(sengoo_async_shared_counter_new_i64),
        export_addr!(sengoo_async_shared_counter_clone_i64),
        export_addr!(sengoo_async_shared_counter_get_i64),
        export_addr!(sengoo_async_shared_counter_drop),
        export_addr!(sengoo_async_shared_counter_spawn_add_i64),
        export_addr!(sengoo_async_shared_counter_join_i64),
        export_addr!(sengoo_async_shared_counter_job_drop),
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
        export_addr!(sengoo_async_mutex_new),
        export_addr!(sengoo_async_mutex_new_parts),
        export_addr!(sengoo_async_mutex_new_i64),
        export_addr!(sengoo_async_mutex_close),
        export_addr!(sengoo_async_mutex_drop),
        export_addr!(sengoo_async_mutex_lock__start),
        export_addr!(sengoo_async_mutex_lock__poll),
        export_addr!(sengoo_async_mutex_lock__result),
        export_addr!(sengoo_async_mutex_lock__cancel),
        export_addr!(sengoo_async_mutex_lock__drop),
        export_addr!(sengoo_async_mutex_lock_i64__start),
        export_addr!(sengoo_async_mutex_lock_i64__poll),
        export_addr!(sengoo_async_mutex_lock_i64__result),
        export_addr!(sengoo_async_mutex_lock_i64__cancel),
        export_addr!(sengoo_async_mutex_lock_i64__drop),
        export_addr!(sengoo_async_mutex_unlock_i64),
        export_addr!(sengoo_async_mutex_guard_copy_into),
        export_addr!(sengoo_async_mutex_guard_get),
        export_addr!(sengoo_async_mutex_guard_set),
        export_addr!(sengoo_async_mutex_guard_unlock),
        export_addr!(sengoo_async_mutex_guard_get_i64),
        export_addr!(sengoo_async_mutex_guard_set_i64),
        export_addr!(sengoo_async_mutex_guard_unlock_i64),
        export_addr!(sengoo_async_rwlock_new_i64),
        export_addr!(sengoo_async_rwlock_close),
        export_addr!(sengoo_async_rwlock_drop),
        export_addr!(sengoo_async_rwlock_try_read_i64),
        export_addr!(sengoo_async_rwlock_try_write_i64),
        export_addr!(sengoo_async_rwlock_read_guard_get_i64),
        export_addr!(sengoo_async_rwlock_write_guard_get_i64),
        export_addr!(sengoo_async_rwlock_write_guard_set_i64),
        export_addr!(sengoo_async_rwlock_read_guard_unlock_i64),
        export_addr!(sengoo_async_rwlock_write_guard_unlock_i64),
    ];
}
