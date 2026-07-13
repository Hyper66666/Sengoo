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

    unsafe fn move_into_output(self, output: *mut c_void) {
        let descriptor = self.descriptor;
        let layout = self.layout;
        let ptr = self.ptr;
        std::mem::forget(self);
        descriptor
            .move_value
            .expect("validated descriptor keeps move")(
            output, ptr.as_ptr().cast::<c_void>()
        );
        dealloc(ptr.as_ptr(), layout);
    }

    unsafe fn replace_initialized_output(self, output: *mut c_void) {
        self.descriptor
            .drop_value
            .expect("validated descriptor keeps drop")(output);
        self.move_into_output(output);
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
    descriptor: Option<SengooTypeDescriptor>,
    queue: VecDeque<ChannelQueueEntry>,
    sender_count: usize,
    receiver_alive: bool,
    closed: bool,
}

enum ChannelQueueEntry {
    I64(i64),
    Descriptor(OwnedDescriptorValue),
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

type ChannelPair = (Option<ChannelSender>, Option<ChannelReceiver>);

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

struct ChannelSendState {
    channel: Arc<ChannelShared>,
    value: Option<OwnedDescriptorValue>,
    lifecycle: PollLifecycle,
    outcome: Option<i64>,
}

struct ChannelRecvState {
    channel: Arc<ChannelShared>,
    lifecycle: PollLifecycle,
    outcome: Option<Result<OwnedDescriptorValue, i64>>,
}

struct ChannelValueHandle {
    payload: Option<OwnedDescriptorValue>,
}

fn channel_drop_sender(channel: &Arc<ChannelShared>) {
    if let Ok(mut inner) = channel.inner.lock() {
        inner.sender_count = inner.sender_count.saturating_sub(1);
        if inner.sender_count == 0 {
            inner.closed = true;
        }
    }
}

fn channel_drop_receiver(channel: &Arc<ChannelShared>) {
    if let Ok(mut inner) = channel.inner.lock() {
        if !inner.receiver_alive {
            return;
        }
        inner.receiver_alive = false;
        inner.queue.clear();
    }
}

impl Drop for ChannelSender {
    fn drop(&mut self) {
        channel_drop_sender(&self.channel);
    }
}

impl Drop for ChannelReceiver {
    fn drop(&mut self) {
        channel_drop_receiver(&self.channel);
    }
}

#[no_mangle]
pub extern "C" fn sengoo_async_channel_bounded_i64(capacity: i64) -> i64 {
    if capacity < 1 {
        return 0;
    }
    let channel = Arc::new(ChannelShared {
        inner: Mutex::new(ChannelInner {
            capacity: capacity as usize,
            descriptor: None,
            queue: VecDeque::new(),
            sender_count: 1,
            receiver_alive: true,
            closed: false,
        }),
    });
    let pair = (
        Some(ChannelSender {
            channel: channel.clone(),
        }),
        Some(ChannelReceiver { channel }),
    );
    Box::into_raw(Box::new(pair)) as i64
}

#[no_mangle]
pub extern "C" fn sengoo_async_channel_bounded_parts(
    capacity: i64,
    size: i64,
    align: i64,
    move_value: Option<SengooMoveFn>,
    drop_value: Option<SengooDropFn>,
) -> i64 {
    if capacity < 1 {
        return 0;
    }
    let Some(descriptor) = descriptor_from_parts(size, align, move_value, drop_value) else {
        return 0;
    };
    let channel = Arc::new(ChannelShared {
        inner: Mutex::new(ChannelInner {
            capacity: capacity as usize,
            descriptor: Some(descriptor),
            queue: VecDeque::new(),
            sender_count: 1,
            receiver_alive: true,
            closed: false,
        }),
    });
    let pair = (
        Some(ChannelSender {
            channel: channel.clone(),
        }),
        Some(ChannelReceiver { channel }),
    );
    Box::into_raw(Box::new(pair)) as i64
}

#[no_mangle]
/// # Safety
///
/// `pair_handle` must be a live handle returned by
/// [`sengoo_async_channel_bounded_i64`].
pub unsafe extern "C" fn sengoo_async_channel_pair_sender(pair_handle: i64) -> i64 {
    let Some(pair) = handle_mut::<ChannelPair>(pair_handle) else {
        return 0;
    };
    let Some(sender) = pair.0.take() else {
        return 0;
    };
    Box::into_raw(Box::new(sender)) as i64
}

#[no_mangle]
/// # Safety
///
/// `pair_handle` must be a live handle returned by
/// [`sengoo_async_channel_bounded_i64`].
pub unsafe extern "C" fn sengoo_async_channel_pair_receiver(pair_handle: i64) -> i64 {
    let Some(pair) = handle_mut::<ChannelPair>(pair_handle) else {
        return 0;
    };
    let Some(receiver) = pair.1.take() else {
        return 0;
    };
    Box::into_raw(Box::new(receiver)) as i64
}

#[no_mangle]
/// # Safety
///
/// `pair_handle` must be a live handle returned by
/// [`sengoo_async_channel_bounded_i64`].
pub unsafe extern "C" fn sengoo_async_channel_pair_free(pair_handle: i64) {
    let Some(pair) = handle_take_box::<ChannelPair>(pair_handle) else {
        return;
    };
    drop(pair);
}

#[no_mangle]
/// # Safety
///
/// `receiver_handle` must be a live handle returned by
/// [`sengoo_async_channel_pair_receiver`].
pub unsafe extern "C" fn sengoo_async_channel_receiver_drop(receiver_handle: i64) {
    let Some(receiver) = handle_take_box::<ChannelReceiver>(receiver_handle) else {
        return;
    };
    drop(receiver);
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
    drop(sender);
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
    let Ok(inner) = sender.channel.inner.lock() else {
        return 0;
    };
    if inner.descriptor.is_some() {
        return 0;
    }
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
    if inner.closed || !inner.receiver_alive {
        state.outcome = Some(Err(STATUS_INVALID_HANDLE));
        guard.mark_ready();
        return 1;
    }
    if inner.queue.len() < inner.capacity {
        inner.queue.push_back(ChannelQueueEntry::I64(state.value));
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

fn drop_failed_channel_send_input(value: *mut c_void, drop_value: Option<SengooDropFn>) {
    if value.is_null() {
        return;
    }
    let Some(drop_value) = drop_value else {
        return;
    };
    unsafe { drop_value(value) };
}

#[no_mangle]
/// # Safety
///
/// `sender_handle` must be a live handle returned by
/// [`sengoo_async_channel_pair_sender`] for a descriptor-backed channel, and
/// `value` must point to an owned payload of the channel element type.
pub unsafe extern "C" fn sengoo_async_channel_send__start(
    sender_handle: i64,
    value: *mut c_void,
    drop_value: Option<SengooDropFn>,
) -> i64 {
    let Some(sender) = handle_ref::<ChannelSender>(sender_handle) else {
        drop_failed_channel_send_input(value, drop_value);
        return 0;
    };
    let descriptor = {
        let Ok(inner) = sender.channel.inner.lock() else {
            drop_failed_channel_send_input(value, drop_value);
            return 0;
        };
        let Some(descriptor) = inner.descriptor else {
            drop_failed_channel_send_input(value, drop_value);
            return 0;
        };
        descriptor
    };
    let Some(value) = OwnedDescriptorValue::new(&descriptor, value) else {
        drop_failed_channel_send_input(value, drop_value);
        return 0;
    };
    let state = ChannelSendState {
        channel: sender.channel.clone(),
        value: Some(value),
        lifecycle: PollLifecycle::default(),
        outcome: None,
    };
    Box::into_raw(Box::new(state)) as i64
}

#[no_mangle]
/// # Safety
///
/// `handle` must be a live handle returned by
/// [`sengoo_async_channel_send__start`].
pub unsafe extern "C" fn sengoo_async_channel_send__poll(handle: i64) -> i64 {
    let Some(state) = handle_mut::<ChannelSendState>(handle) else {
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
    if inner.closed || !inner.receiver_alive || inner.descriptor.is_none() {
        state.outcome = Some(-STATUS_INVALID_HANDLE);
        guard.mark_ready();
        return 1;
    }
    if inner.queue.len() < inner.capacity {
        let Some(value) = state.value.take() else {
            state.outcome = Some(-STATUS_INVALID_HANDLE);
            guard.mark_ready();
            return 1;
        };
        inner.queue.push_back(ChannelQueueEntry::Descriptor(value));
        state.outcome = Some(0);
        guard.mark_ready();
        return 1;
    }
    record_poll_wakeup_hint(std::time::Instant::now());
    0
}

#[no_mangle]
/// # Safety
///
/// `handle` must be an unconsumed handle returned by
/// [`sengoo_async_channel_send__start`].
pub unsafe extern "C" fn sengoo_async_channel_send__result(handle: i64) -> i64 {
    let Some(state) = handle_take_box::<ChannelSendState>(handle) else {
        return -STATUS_INVALID_HANDLE;
    };
    state.outcome.unwrap_or(-STATUS_INVALID_HANDLE)
}

#[no_mangle]
/// # Safety
///
/// `handle` must be an unconsumed handle returned by
/// [`sengoo_async_channel_send__start`].
pub unsafe extern "C" fn sengoo_async_channel_send__cancel(handle: i64) -> bool {
    let Some(state) = handle_take_box::<ChannelSendState>(handle) else {
        return false;
    };
    drop(state);
    true
}

#[no_mangle]
/// # Safety
///
/// `handle` must be an unconsumed handle returned by
/// [`sengoo_async_channel_send__start`].
pub unsafe extern "C" fn sengoo_async_channel_send__drop(handle: i64) {
    let Some(state) = handle_take_box::<ChannelSendState>(handle) else {
        return;
    };
    drop(state);
}

#[no_mangle]
pub extern "C" fn sengoo_async_channel_recv_i64__start(receiver_handle: i64) -> i64 {
    let Some(receiver) = (unsafe { handle_ref::<ChannelReceiver>(receiver_handle) }) else {
        return 0;
    };
    let Ok(inner) = receiver.channel.inner.lock() else {
        return 0;
    };
    if inner.descriptor.is_some() {
        return 0;
    }
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
    if let Some(entry) = inner.queue.pop_front() {
        match entry {
            ChannelQueueEntry::I64(value) => {
                state.outcome = Some(Ok(value));
                guard.mark_ready();
                return 1;
            }
            ChannelQueueEntry::Descriptor(value) => {
                inner.queue.push_front(ChannelQueueEntry::Descriptor(value));
                state.outcome = Some(Err(STATUS_INVALID_HANDLE));
                guard.mark_ready();
                return 1;
            }
        }
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

#[no_mangle]
pub extern "C" fn sengoo_async_channel_recv__start(receiver_handle: i64) -> i64 {
    let Some(receiver) = (unsafe { handle_ref::<ChannelReceiver>(receiver_handle) }) else {
        return 0;
    };
    let Ok(inner) = receiver.channel.inner.lock() else {
        return 0;
    };
    if inner.descriptor.is_none() {
        return 0;
    }
    let state = ChannelRecvState {
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
/// [`sengoo_async_channel_recv__start`].
pub unsafe extern "C" fn sengoo_async_channel_recv__poll(handle: i64) -> i64 {
    let Some(state) = handle_mut::<ChannelRecvState>(handle) else {
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
    if let Some(entry) = inner.queue.pop_front() {
        match entry {
            ChannelQueueEntry::Descriptor(value) => {
                state.outcome = Some(Ok(value));
                guard.mark_ready();
                return 1;
            }
            ChannelQueueEntry::I64(value) => {
                inner.queue.push_front(ChannelQueueEntry::I64(value));
                state.outcome = Some(Err(-STATUS_INVALID_HANDLE));
                guard.mark_ready();
                return 1;
            }
        }
    }
    if inner.closed {
        state.outcome = Some(Err(-STATUS_INVALID_HANDLE));
        guard.mark_ready();
        return 1;
    }
    record_poll_wakeup_hint(std::time::Instant::now());
    0
}

#[no_mangle]
/// # Safety
///
/// `handle` must be an unconsumed handle returned by
/// [`sengoo_async_channel_recv__start`].
pub unsafe extern "C" fn sengoo_async_channel_recv__result(handle: i64) -> i64 {
    let Some(state) = handle_take_box::<ChannelRecvState>(handle) else {
        return -STATUS_INVALID_HANDLE;
    };
    match state.outcome {
        Some(Ok(payload)) => Box::into_raw(Box::new(ChannelValueHandle {
            payload: Some(payload),
        })) as i64,
        Some(Err(code)) => code,
        None => -STATUS_INVALID_HANDLE,
    }
}

#[no_mangle]
/// # Safety
///
/// `handle` must be an unconsumed handle returned by
/// [`sengoo_async_channel_recv__start`].
pub unsafe extern "C" fn sengoo_async_channel_recv__cancel(handle: i64) -> bool {
    let Some(state) = handle_take_box::<ChannelRecvState>(handle) else {
        return false;
    };
    drop(state);
    true
}

#[no_mangle]
/// # Safety
///
/// `handle` must be an unconsumed handle returned by
/// [`sengoo_async_channel_recv__start`].
pub unsafe extern "C" fn sengoo_async_channel_recv__drop(handle: i64) {
    let Some(state) = handle_take_box::<ChannelRecvState>(handle) else {
        return;
    };
    drop(state);
}

#[no_mangle]
/// # Safety
///
/// `value_handle` must be a live handle returned by
/// [`sengoo_async_channel_recv__result`] with a positive result, and
/// `initialized_output_ptr` must point to an initialized value of the same
/// descriptor-backed type.
pub unsafe extern "C" fn sengoo_async_channel_value_move_into(
    value_handle: i64,
    initialized_output_ptr: *mut c_void,
) -> i64 {
    let Some(mut state) = handle_take_box::<ChannelValueHandle>(value_handle) else {
        return -STATUS_INVALID_HANDLE;
    };
    if initialized_output_ptr.is_null() {
        return -STATUS_INVALID_ARGUMENT;
    }
    let Some(payload) = state.payload.take() else {
        return -STATUS_INVALID_HANDLE;
    };
    payload.replace_initialized_output(initialized_output_ptr);
    0
}

#[no_mangle]
/// # Safety
///
/// `value_handle` must be a live handle returned by
/// [`sengoo_async_channel_recv__result`] with a positive result.
pub unsafe extern "C" fn sengoo_async_channel_value_drop(value_handle: i64) {
    let Some(state) = handle_take_box::<ChannelValueHandle>(value_handle) else {
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
    closed: bool,
    next_guard_id: i64,
    next_waiter_id: i64,
    guards: HashMap<i64, RwLockGuardKind>,
    waiters: HashMap<i64, RwLockWaiter>,
    waiter_queue: VecDeque<i64>,
    payload: OwnedDescriptorValue,
}

struct AsyncRwLockState {
    inner: Arc<Mutex<AsyncRwLockInner>>,
}

#[derive(Clone, Copy)]
struct RwLockWaiter {
    kind: RwLockGuardKind,
}

struct RwLockAcquireState {
    lock: Arc<Mutex<AsyncRwLockInner>>,
    kind: RwLockGuardKind,
    lifecycle: PollLifecycle,
    outcome: Option<i64>,
    waiter_id: Option<i64>,
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

fn allocate_rwlock_waiter_id(inner: &mut AsyncRwLockInner) -> i64 {
    loop {
        let candidate = inner.next_waiter_id.max(1);
        inner.next_waiter_id = if candidate == i64::MAX {
            1
        } else {
            candidate + 1
        };
        if !inner.waiters.contains_key(&candidate) {
            return candidate;
        }
    }
}

fn rwlock_has_waiting_writer(inner: &AsyncRwLockInner) -> bool {
    inner
        .waiters
        .values()
        .any(|waiter| waiter.kind == RwLockGuardKind::Write)
}

fn rwlock_first_waiter_id(inner: &AsyncRwLockInner) -> Option<i64> {
    inner
        .waiter_queue
        .iter()
        .copied()
        .find(|waiter_id| inner.waiters.contains_key(waiter_id))
}

fn rwlock_register_waiter(inner: &mut AsyncRwLockInner, kind: RwLockGuardKind) -> i64 {
    let waiter_id = allocate_rwlock_waiter_id(inner);
    inner.waiters.insert(waiter_id, RwLockWaiter { kind });
    inner.waiter_queue.push_back(waiter_id);
    waiter_id
}

fn rwlock_unregister_waiter(inner: &mut AsyncRwLockInner, waiter_id: i64) -> bool {
    let removed = inner.waiters.remove(&waiter_id).is_some();
    if removed {
        if let Some(index) = inner
            .waiter_queue
            .iter()
            .position(|queued_waiter| *queued_waiter == waiter_id)
        {
            inner.waiter_queue.remove(index);
        }
    }
    removed
}

fn rwlock_acquire_guard(inner: &mut AsyncRwLockInner, kind: RwLockGuardKind) -> i64 {
    let guard_id = allocate_rwlock_guard_id(inner);
    match kind {
        RwLockGuardKind::Read => inner.readers += 1,
        RwLockGuardKind::Write => inner.writer = true,
    }
    inner.guards.insert(guard_id, kind);
    guard_id
}

fn rwlock_waiter_can_acquire(inner: &AsyncRwLockInner, waiter_id: i64) -> bool {
    let Some(waiter) = inner.waiters.get(&waiter_id) else {
        return false;
    };
    if rwlock_first_waiter_id(inner) != Some(waiter_id) {
        return false;
    }
    match waiter.kind {
        RwLockGuardKind::Read => !inner.writer,
        RwLockGuardKind::Write => !inner.writer && inner.readers == 0,
    }
}

#[no_mangle]
/// # Safety
///
/// `descriptor` must describe the owned payload pointed to by `value`.
pub unsafe extern "C" fn sengoo_async_rwlock_new(
    descriptor: *const SengooTypeDescriptor,
    value: *mut c_void,
) -> i64 {
    let Some(descriptor) = descriptor.as_ref() else {
        return 0;
    };
    let Some(payload) = OwnedDescriptorValue::new(descriptor, value) else {
        return 0;
    };
    let state = AsyncRwLockState {
        inner: Arc::new(Mutex::new(AsyncRwLockInner {
            readers: 0,
            writer: false,
            closed: false,
            next_guard_id: 1,
            next_waiter_id: 1,
            guards: HashMap::new(),
            waiters: HashMap::new(),
            waiter_queue: VecDeque::new(),
            payload,
        })),
    };
    Box::into_raw(Box::new(state)) as i64
}

#[no_mangle]
/// # Safety
///
/// `value` must point to an owned payload compatible with the supplied parts.
pub unsafe extern "C" fn sengoo_async_rwlock_new_parts(
    value: *mut c_void,
    size: i64,
    align: i64,
    move_value: Option<SengooMoveFn>,
    drop_value: Option<SengooDropFn>,
) -> i64 {
    let Some(descriptor) = descriptor_from_parts(size, align, move_value, drop_value) else {
        return 0;
    };
    unsafe { sengoo_async_rwlock_new(&descriptor, value) }
}

#[no_mangle]
pub extern "C" fn sengoo_async_rwlock_new_i64(value: i64) -> i64 {
    let descriptor = scalar_i64_descriptor();
    let mut slot = value;
    unsafe { sengoo_async_rwlock_new(&descriptor, (&mut slot as *mut i64).cast::<c_void>()) }
}

#[no_mangle]
/// # Safety
///
/// `handle` must be a live handle returned by [`sengoo_async_rwlock_new`].
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
/// `handle` must be a live handle returned by [`sengoo_async_rwlock_new`]
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
/// Attempts to acquire a generic read guard without blocking.
///
/// Returns a positive guard token on success, `-STATUS_LOCK_UNAVAILABLE` when a
/// writer owns the lock, or `-STATUS_INVALID_HANDLE` for a closed/invalid lock.
///
/// # Safety
///
/// `lock_handle` must be a live handle returned by
/// [`sengoo_async_rwlock_new`].
pub unsafe extern "C" fn sengoo_async_rwlock_try_read(lock_handle: i64) -> i64 {
    let Some(state) = handle_ref::<AsyncRwLockState>(lock_handle) else {
        return -STATUS_INVALID_HANDLE;
    };
    let Ok(mut inner) = state.inner.lock() else {
        return -STATUS_INVALID_HANDLE;
    };
    if inner.closed {
        return -STATUS_INVALID_HANDLE;
    }
    if inner.writer || rwlock_has_waiting_writer(&inner) {
        return -STATUS_LOCK_UNAVAILABLE;
    }
    rwlock_acquire_guard(&mut inner, RwLockGuardKind::Read)
}

#[no_mangle]
/// Attempts to acquire a generic write guard without blocking.
///
/// Returns a positive guard token on success, `-STATUS_LOCK_UNAVAILABLE` while
/// any reader or writer owns the lock, or `-STATUS_INVALID_HANDLE` for a
/// closed/invalid lock.
///
/// # Safety
///
/// `lock_handle` must be a live handle returned by
/// [`sengoo_async_rwlock_new`].
pub unsafe extern "C" fn sengoo_async_rwlock_try_write(lock_handle: i64) -> i64 {
    let Some(state) = handle_ref::<AsyncRwLockState>(lock_handle) else {
        return -STATUS_INVALID_HANDLE;
    };
    let Ok(mut inner) = state.inner.lock() else {
        return -STATUS_INVALID_HANDLE;
    };
    if inner.closed {
        return -STATUS_INVALID_HANDLE;
    }
    if inner.writer || inner.readers != 0 || !inner.waiters.is_empty() {
        return -STATUS_LOCK_UNAVAILABLE;
    }
    rwlock_acquire_guard(&mut inner, RwLockGuardKind::Write)
}

#[no_mangle]
pub extern "C" fn sengoo_async_rwlock_read__start(lock_handle: i64) -> i64 {
    let Some(lock_state) = (unsafe { handle_ref::<AsyncRwLockState>(lock_handle) }) else {
        return 0;
    };
    let state = RwLockAcquireState {
        lock: lock_state.inner.clone(),
        kind: RwLockGuardKind::Read,
        lifecycle: PollLifecycle::default(),
        outcome: None,
        waiter_id: None,
    };
    Box::into_raw(Box::new(state)) as i64
}

#[no_mangle]
pub extern "C" fn sengoo_async_rwlock_write__start(lock_handle: i64) -> i64 {
    let Some(lock_state) = (unsafe { handle_ref::<AsyncRwLockState>(lock_handle) }) else {
        return 0;
    };
    let state = RwLockAcquireState {
        lock: lock_state.inner.clone(),
        kind: RwLockGuardKind::Write,
        lifecycle: PollLifecycle::default(),
        outcome: None,
        waiter_id: None,
    };
    Box::into_raw(Box::new(state)) as i64
}

fn rwlock_acquire_poll(state: &mut RwLockAcquireState) -> i64 {
    let guard = match state.lifecycle.enter() {
        Ok(guard) => guard,
        Err(error) => return error,
    };
    if state.outcome.is_some() {
        guard.mark_ready();
        return 1;
    }

    let mut inner = state.lock.lock().expect("async rwlock inner poisoned");
    if inner.closed {
        if let Some(waiter_id) = state.waiter_id.take() {
            rwlock_unregister_waiter(&mut inner, waiter_id);
        }
        state.outcome = Some(-STATUS_INVALID_HANDLE);
        guard.mark_ready();
        return 1;
    }

    if let Some(waiter_id) = state.waiter_id {
        if rwlock_waiter_can_acquire(&inner, waiter_id) {
            rwlock_unregister_waiter(&mut inner, waiter_id);
            state.waiter_id = None;
            state.outcome = Some(rwlock_acquire_guard(&mut inner, state.kind));
            guard.mark_ready();
            return 1;
        }
    } else {
        let can_acquire = match state.kind {
            RwLockGuardKind::Read => !inner.writer && !rwlock_has_waiting_writer(&inner),
            RwLockGuardKind::Write => {
                !inner.writer && inner.readers == 0 && rwlock_first_waiter_id(&inner).is_none()
            }
        };
        if can_acquire {
            state.outcome = Some(rwlock_acquire_guard(&mut inner, state.kind));
            guard.mark_ready();
            return 1;
        }
        state.waiter_id = Some(rwlock_register_waiter(&mut inner, state.kind));
    }

    record_poll_wakeup_hint(std::time::Instant::now());
    0
}

#[no_mangle]
/// # Safety
///
/// `handle` must be zero or a live handle returned by
/// [`sengoo_async_rwlock_read__start`].
pub unsafe extern "C" fn sengoo_async_rwlock_read__poll(handle: i64) -> i64 {
    let Some(state) = handle_mut::<RwLockAcquireState>(handle) else {
        return 1;
    };
    if state.kind != RwLockGuardKind::Read {
        return -STATUS_INVALID_HANDLE;
    }
    rwlock_acquire_poll(state)
}

#[no_mangle]
/// # Safety
///
/// `handle` must be zero or a live handle returned by
/// [`sengoo_async_rwlock_write__start`].
pub unsafe extern "C" fn sengoo_async_rwlock_write__poll(handle: i64) -> i64 {
    let Some(state) = handle_mut::<RwLockAcquireState>(handle) else {
        return 1;
    };
    if state.kind != RwLockGuardKind::Write {
        return -STATUS_INVALID_HANDLE;
    }
    rwlock_acquire_poll(state)
}

fn rwlock_acquire_result(handle: i64, expected: RwLockGuardKind) -> i64 {
    let Some(mut state) = (unsafe { handle_take_box::<RwLockAcquireState>(handle) }) else {
        return -STATUS_INVALID_HANDLE;
    };
    if state.kind != expected {
        return -STATUS_INVALID_HANDLE;
    }
    if let Some(waiter_id) = state.waiter_id.take() {
        if let Ok(mut inner) = state.lock.lock() {
            rwlock_unregister_waiter(&mut inner, waiter_id);
        }
    }
    state.outcome.unwrap_or(-STATUS_INVALID_HANDLE)
}

#[no_mangle]
/// # Safety
///
/// `handle` must be zero or an unconsumed handle returned by
/// [`sengoo_async_rwlock_read__start`].
pub unsafe extern "C" fn sengoo_async_rwlock_read__result(handle: i64) -> i64 {
    rwlock_acquire_result(handle, RwLockGuardKind::Read)
}

#[no_mangle]
/// # Safety
///
/// `handle` must be zero or an unconsumed handle returned by
/// [`sengoo_async_rwlock_write__start`].
pub unsafe extern "C" fn sengoo_async_rwlock_write__result(handle: i64) -> i64 {
    rwlock_acquire_result(handle, RwLockGuardKind::Write)
}

fn rwlock_acquire_cancel(handle: i64, expected: RwLockGuardKind) -> bool {
    let Some(mut state) = (unsafe { handle_take_box::<RwLockAcquireState>(handle) }) else {
        return false;
    };
    if state.kind != expected {
        return false;
    }
    if let Some(waiter_id) = state.waiter_id.take() {
        if let Ok(mut inner) = state.lock.lock() {
            rwlock_unregister_waiter(&mut inner, waiter_id);
        }
    }
    drop(state);
    true
}

#[no_mangle]
/// # Safety
///
/// `handle` must be zero or an unconsumed handle returned by
/// [`sengoo_async_rwlock_read__start`].
pub unsafe extern "C" fn sengoo_async_rwlock_read__cancel(handle: i64) -> bool {
    rwlock_acquire_cancel(handle, RwLockGuardKind::Read)
}

#[no_mangle]
/// # Safety
///
/// `handle` must be zero or an unconsumed handle returned by
/// [`sengoo_async_rwlock_write__start`].
pub unsafe extern "C" fn sengoo_async_rwlock_write__cancel(handle: i64) -> bool {
    rwlock_acquire_cancel(handle, RwLockGuardKind::Write)
}

fn rwlock_acquire_drop(handle: i64, expected: RwLockGuardKind) {
    let Some(mut state) = (unsafe { handle_take_box::<RwLockAcquireState>(handle) }) else {
        return;
    };
    if state.kind != expected {
        return;
    }
    if let Some(waiter_id) = state.waiter_id.take() {
        if let Ok(mut inner) = state.lock.lock() {
            rwlock_unregister_waiter(&mut inner, waiter_id);
        }
    }
    drop(state);
}

#[no_mangle]
/// # Safety
///
/// `handle` must be zero or an unconsumed handle returned by
/// [`sengoo_async_rwlock_read__start`].
pub unsafe extern "C" fn sengoo_async_rwlock_read__drop(handle: i64) {
    rwlock_acquire_drop(handle, RwLockGuardKind::Read);
}

#[no_mangle]
/// # Safety
///
/// `handle` must be zero or an unconsumed handle returned by
/// [`sengoo_async_rwlock_write__start`].
pub unsafe extern "C" fn sengoo_async_rwlock_write__drop(handle: i64) {
    rwlock_acquire_drop(handle, RwLockGuardKind::Write);
}

fn rwlock_guard_copy_into(
    state: &AsyncRwLockState,
    guard_id: i64,
    expected: RwLockGuardKind,
    output: *mut c_void,
    size: i64,
) -> i64 {
    if output.is_null() || size <= 0 {
        return -STATUS_INVALID_ARGUMENT;
    }
    let Ok(inner) = state.inner.lock() else {
        return -STATUS_INVALID_HANDLE;
    };
    if inner.guards.get(&guard_id) != Some(&expected) {
        return -STATUS_INVALID_HANDLE;
    }
    if size as usize != inner.payload.descriptor.size {
        return -STATUS_INVALID_ARGUMENT;
    }
    unsafe {
        std::ptr::copy_nonoverlapping(
            inner.payload.as_ptr().cast::<u8>(),
            output.cast::<u8>(),
            size as usize,
        );
    }
    0
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
pub unsafe extern "C" fn sengoo_async_rwlock_read_guard_copy_into(
    lock_handle: i64,
    guard_id: i64,
    output: *mut c_void,
    size: i64,
) -> i64 {
    let Some(state) = handle_ref::<AsyncRwLockState>(lock_handle) else {
        return -STATUS_INVALID_HANDLE;
    };
    rwlock_guard_copy_into(state, guard_id, RwLockGuardKind::Read, output, size)
}

#[no_mangle]
/// # Safety
///
/// `lock_handle` must identify a live rwlock and `guard_id` must identify one
/// of its active write guards.
pub unsafe extern "C" fn sengoo_async_rwlock_write_guard_copy_into(
    lock_handle: i64,
    guard_id: i64,
    output: *mut c_void,
    size: i64,
) -> i64 {
    let Some(state) = handle_ref::<AsyncRwLockState>(lock_handle) else {
        return -STATUS_INVALID_HANDLE;
    };
    rwlock_guard_copy_into(state, guard_id, RwLockGuardKind::Write, output, size)
}

#[no_mangle]
/// # Safety
///
/// `lock_handle` must identify a live rwlock and `guard_id` must identify one
/// of its active write guards. When `value` is non-null and `drop_value` is
/// present, this function always consumes the payload: successful replacement
/// moves it into the lock and every failure path drops it through `drop_value`.
pub unsafe extern "C" fn sengoo_async_rwlock_write_guard_set(
    lock_handle: i64,
    guard_id: i64,
    value: *mut c_void,
    drop_value: Option<SengooDropFn>,
) -> i64 {
    if value.is_null() {
        return -STATUS_INVALID_ARGUMENT;
    }
    let Some(drop_value) = drop_value else {
        return -STATUS_INVALID_ARGUMENT;
    };
    let Some(state) = handle_ref::<AsyncRwLockState>(lock_handle) else {
        drop_value(value);
        return -STATUS_INVALID_HANDLE;
    };
    let Ok(inner) = state.inner.lock() else {
        drop_value(value);
        return -STATUS_INVALID_HANDLE;
    };
    if inner.guards.get(&guard_id) != Some(&RwLockGuardKind::Write) {
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
/// `lock_handle` must identify a live rwlock. Releasing an already released or
/// mismatched guard token safely returns `-STATUS_INVALID_HANDLE`.
pub unsafe extern "C" fn sengoo_async_rwlock_read_guard_unlock(
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
pub unsafe extern "C" fn sengoo_async_rwlock_write_guard_unlock(
    lock_handle: i64,
    guard_id: i64,
) -> i64 {
    let Some(state) = handle_ref::<AsyncRwLockState>(lock_handle) else {
        return -STATUS_INVALID_HANDLE;
    };
    rwlock_guard_unlock(state, guard_id, RwLockGuardKind::Write)
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
    let mut value = 0_i64;
    if unsafe {
        sengoo_async_rwlock_read_guard_copy_into(
            lock_handle,
            guard_id,
            (&mut value as *mut i64).cast::<c_void>(),
            std::mem::size_of::<i64>() as i64,
        )
    } == 0
    {
        value
    } else {
        0
    }
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
    let mut value = 0_i64;
    if unsafe {
        sengoo_async_rwlock_write_guard_copy_into(
            lock_handle,
            guard_id,
            (&mut value as *mut i64).cast::<c_void>(),
            std::mem::size_of::<i64>() as i64,
        )
    } == 0
    {
        value
    } else {
        0
    }
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
    let mut slot = value;
    unsafe {
        sengoo_async_rwlock_write_guard_set(
            lock_handle,
            guard_id,
            (&mut slot as *mut i64).cast::<c_void>(),
            Some(sengoo_scalar_drop_i64),
        )
    }
}

#[no_mangle]
/// # Safety
///
/// `lock_handle` must be a live handle returned by [`sengoo_async_rwlock_new`].
pub unsafe extern "C" fn sengoo_async_rwlock_try_read_i64(lock_handle: i64) -> i64 {
    unsafe { sengoo_async_rwlock_try_read(lock_handle) }
}

#[no_mangle]
pub extern "C" fn sengoo_async_rwlock_read_i64__start(lock_handle: i64) -> i64 {
    sengoo_async_rwlock_read__start(lock_handle)
}

#[no_mangle]
pub extern "C" fn sengoo_async_rwlock_write_i64__start(lock_handle: i64) -> i64 {
    sengoo_async_rwlock_write__start(lock_handle)
}

#[no_mangle]
/// # Safety
///
/// `handle` must be zero or a live handle returned by
/// [`sengoo_async_rwlock_read_i64__start`].
pub unsafe extern "C" fn sengoo_async_rwlock_read_i64__poll(handle: i64) -> i64 {
    unsafe { sengoo_async_rwlock_read__poll(handle) }
}

#[no_mangle]
/// # Safety
///
/// `handle` must be zero or a live handle returned by
/// [`sengoo_async_rwlock_write_i64__start`].
pub unsafe extern "C" fn sengoo_async_rwlock_write_i64__poll(handle: i64) -> i64 {
    unsafe { sengoo_async_rwlock_write__poll(handle) }
}

#[no_mangle]
/// # Safety
///
/// `handle` must be zero or an unconsumed handle returned by
/// [`sengoo_async_rwlock_read_i64__start`].
pub unsafe extern "C" fn sengoo_async_rwlock_read_i64__result(handle: i64) -> i64 {
    unsafe { sengoo_async_rwlock_read__result(handle) }
}

#[no_mangle]
/// # Safety
///
/// `handle` must be zero or an unconsumed handle returned by
/// [`sengoo_async_rwlock_write_i64__start`].
pub unsafe extern "C" fn sengoo_async_rwlock_write_i64__result(handle: i64) -> i64 {
    unsafe { sengoo_async_rwlock_write__result(handle) }
}

#[no_mangle]
/// # Safety
///
/// `handle` must be zero or an unconsumed handle returned by
/// [`sengoo_async_rwlock_read_i64__start`].
pub unsafe extern "C" fn sengoo_async_rwlock_read_i64__cancel(handle: i64) -> bool {
    unsafe { sengoo_async_rwlock_read__cancel(handle) }
}

#[no_mangle]
/// # Safety
///
/// `handle` must be zero or an unconsumed handle returned by
/// [`sengoo_async_rwlock_write_i64__start`].
pub unsafe extern "C" fn sengoo_async_rwlock_write_i64__cancel(handle: i64) -> bool {
    unsafe { sengoo_async_rwlock_write__cancel(handle) }
}

#[no_mangle]
/// # Safety
///
/// `handle` must be zero or an unconsumed handle returned by
/// [`sengoo_async_rwlock_read_i64__start`].
pub unsafe extern "C" fn sengoo_async_rwlock_read_i64__drop(handle: i64) {
    unsafe { sengoo_async_rwlock_read__drop(handle) };
}

#[no_mangle]
/// # Safety
///
/// `handle` must be zero or an unconsumed handle returned by
/// [`sengoo_async_rwlock_write_i64__start`].
pub unsafe extern "C" fn sengoo_async_rwlock_write_i64__drop(handle: i64) {
    unsafe { sengoo_async_rwlock_write__drop(handle) };
}

#[no_mangle]
/// # Safety
///
/// `lock_handle` must be a live handle returned by [`sengoo_async_rwlock_new`].
pub unsafe extern "C" fn sengoo_async_rwlock_try_write_i64(lock_handle: i64) -> i64 {
    unsafe { sengoo_async_rwlock_try_write(lock_handle) }
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
    unsafe { sengoo_async_rwlock_read_guard_unlock(lock_handle, guard_id) }
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
    unsafe { sengoo_async_rwlock_write_guard_unlock(lock_handle, guard_id) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    #[repr(C)]
    struct DropTrackedPayload {
        tag: i64,
        drop_counter: *const AtomicU32,
    }

    unsafe extern "C" fn drop_tracked_move(destination: *mut c_void, source: *mut c_void) {
        std::ptr::copy_nonoverlapping(
            source.cast::<u8>(),
            destination.cast::<u8>(),
            std::mem::size_of::<DropTrackedPayload>(),
        );
        std::ptr::write_bytes(
            source.cast::<u8>(),
            0,
            std::mem::size_of::<DropTrackedPayload>(),
        );
    }

    unsafe extern "C" fn drop_tracked_drop(value: *mut c_void) {
        if value.is_null() {
            return;
        }
        let payload = &*value.cast::<DropTrackedPayload>();
        if let Some(counter) = payload.drop_counter.as_ref() {
            counter.fetch_add(1, Ordering::SeqCst);
        }
    }

    fn drop_tracked_payload(tag: i64, drop_counter: &AtomicU32) -> DropTrackedPayload {
        DropTrackedPayload {
            tag,
            drop_counter: drop_counter as *const AtomicU32,
        }
    }

    fn cleanup_channel_handles(pair: i64, sender: i64, receiver: i64) {
        unsafe {
            drop(handle_take_box::<ChannelSender>(sender));
            sengoo_async_channel_receiver_drop(receiver);
            drop(handle_take_box::<ChannelPair>(pair));
        }
    }

    fn rwlock_waiter_counts(lock_handle: i64) -> (usize, usize) {
        let state = unsafe { handle_ref::<AsyncRwLockState>(lock_handle) }
            .expect("rwlock test handle should stay live");
        let inner = state
            .inner
            .lock()
            .expect("rwlock test inner mutex should stay live");
        let mut read_waiters = 0;
        let mut write_waiters = 0;
        for waiter in inner.waiters.values() {
            match waiter.kind {
                RwLockGuardKind::Read => read_waiters += 1,
                RwLockGuardKind::Write => write_waiters += 1,
            }
        }
        (read_waiters, write_waiters)
    }

    #[test]
    fn async_rwlock_pending_read_waits_behind_writer_and_completes_after_unlock() {
        let lock = sengoo_async_rwlock_new_i64(17);
        let writer = unsafe { sengoo_async_rwlock_try_write(lock) };
        assert!(writer > 0);

        let pending_read = sengoo_async_rwlock_read_i64__start(lock);
        assert!(pending_read > 0);
        assert_eq!(
            unsafe { sengoo_async_rwlock_read_i64__poll(pending_read) },
            0
        );
        assert_eq!(rwlock_waiter_counts(lock), (1, 0));
        assert_eq!(
            unsafe { sengoo_async_rwlock_read_i64__result(0) },
            -STATUS_INVALID_HANDLE
        );

        assert_eq!(
            unsafe { sengoo_async_rwlock_write_guard_unlock_i64(lock, writer) },
            0
        );
        assert_eq!(
            unsafe { sengoo_async_rwlock_read_i64__poll(pending_read) },
            1
        );

        let read = unsafe { sengoo_async_rwlock_read_i64__result(pending_read) };
        assert!(read > 0);
        assert_eq!(
            unsafe { sengoo_async_rwlock_read_guard_get_i64(lock, read) },
            17
        );
        assert_eq!(
            unsafe { sengoo_async_rwlock_read_guard_unlock_i64(lock, read) },
            0
        );

        unsafe { sengoo_async_rwlock_drop(lock) };
    }

    #[test]
    fn async_rwlock_premature_result_unregisters_pending_waiter() {
        let lock = sengoo_async_rwlock_new_i64(19);
        let writer = unsafe { sengoo_async_rwlock_try_write(lock) };
        assert!(writer > 0);

        let pending_read = sengoo_async_rwlock_read_i64__start(lock);
        assert_eq!(
            unsafe { sengoo_async_rwlock_read_i64__poll(pending_read) },
            0
        );
        assert_eq!(rwlock_waiter_counts(lock), (1, 0));

        assert_eq!(
            unsafe { sengoo_async_rwlock_read_i64__result(pending_read) },
            -STATUS_INVALID_HANDLE
        );
        assert_eq!(
            rwlock_waiter_counts(lock),
            (0, 0),
            "consuming a pending acquire result must not strand its waiter"
        );

        assert_eq!(
            unsafe { sengoo_async_rwlock_write_guard_unlock_i64(lock, writer) },
            0
        );
        let next = unsafe { sengoo_async_rwlock_try_write_i64(lock) };
        assert!(next > 0);
        assert_eq!(
            unsafe { sengoo_async_rwlock_write_guard_unlock_i64(lock, next) },
            0
        );
        unsafe { sengoo_async_rwlock_drop(lock) };
    }

    #[test]
    fn async_rwlock_pending_write_waits_behind_readers_and_completes_after_last_reader_unlocks() {
        let lock = sengoo_async_rwlock_new_i64(23);
        let first_reader = unsafe { sengoo_async_rwlock_try_read_i64(lock) };
        let second_reader = unsafe { sengoo_async_rwlock_try_read_i64(lock) };
        assert!(first_reader > 0);
        assert!(second_reader > 0);

        let pending_write = sengoo_async_rwlock_write_i64__start(lock);
        assert!(pending_write > 0);
        assert_eq!(
            unsafe { sengoo_async_rwlock_write_i64__poll(pending_write) },
            0
        );
        assert_eq!(rwlock_waiter_counts(lock), (0, 1));

        assert_eq!(
            unsafe { sengoo_async_rwlock_read_guard_unlock_i64(lock, first_reader) },
            0
        );
        assert_eq!(
            unsafe { sengoo_async_rwlock_write_i64__poll(pending_write) },
            0
        );
        assert_eq!(
            unsafe { sengoo_async_rwlock_read_guard_unlock_i64(lock, second_reader) },
            0
        );
        assert_eq!(
            unsafe { sengoo_async_rwlock_write_i64__poll(pending_write) },
            1
        );

        let writer = unsafe { sengoo_async_rwlock_write_i64__result(pending_write) };
        assert!(writer > 0);
        assert_eq!(
            unsafe { sengoo_async_rwlock_write_guard_get_i64(lock, writer) },
            23
        );
        assert_eq!(
            unsafe { sengoo_async_rwlock_write_guard_set_i64(lock, writer, 29) },
            0
        );
        assert_eq!(
            unsafe { sengoo_async_rwlock_write_guard_unlock_i64(lock, writer) },
            0
        );

        let read_back = unsafe { sengoo_async_rwlock_try_read_i64(lock) };
        assert!(read_back > 0);
        assert_eq!(
            unsafe { sengoo_async_rwlock_read_guard_get_i64(lock, read_back) },
            29
        );
        assert_eq!(
            unsafe { sengoo_async_rwlock_read_guard_unlock_i64(lock, read_back) },
            0
        );

        unsafe { sengoo_async_rwlock_drop(lock) };
    }

    #[test]
    fn async_rwlock_writer_queue_blocks_late_readers_until_writer_runs() {
        let lock = sengoo_async_rwlock_new_i64(31);
        let initial_reader = unsafe { sengoo_async_rwlock_try_read_i64(lock) };
        assert!(initial_reader > 0);

        let pending_write = sengoo_async_rwlock_write_i64__start(lock);
        assert_eq!(
            unsafe { sengoo_async_rwlock_write_i64__poll(pending_write) },
            0
        );
        assert_eq!(rwlock_waiter_counts(lock), (0, 1));

        let late_read = sengoo_async_rwlock_read_i64__start(lock);
        assert_eq!(unsafe { sengoo_async_rwlock_read_i64__poll(late_read) }, 0);
        assert_eq!(rwlock_waiter_counts(lock), (1, 1));

        assert_eq!(
            unsafe { sengoo_async_rwlock_read_guard_unlock_i64(lock, initial_reader) },
            0
        );
        assert_eq!(
            unsafe { sengoo_async_rwlock_write_i64__poll(pending_write) },
            1
        );
        assert_eq!(unsafe { sengoo_async_rwlock_read_i64__poll(late_read) }, 0);

        let writer = unsafe { sengoo_async_rwlock_write_i64__result(pending_write) };
        assert!(writer > 0);
        assert_eq!(
            unsafe { sengoo_async_rwlock_write_guard_unlock_i64(lock, writer) },
            0
        );

        assert_eq!(unsafe { sengoo_async_rwlock_read_i64__poll(late_read) }, 1);
        let final_reader = unsafe { sengoo_async_rwlock_read_i64__result(late_read) };
        assert!(final_reader > 0);
        assert_eq!(
            unsafe { sengoo_async_rwlock_read_guard_unlock_i64(lock, final_reader) },
            0
        );

        unsafe { sengoo_async_rwlock_drop(lock) };
    }

    #[test]
    fn async_rwlock_cancel_and_drop_unregister_waiters_exactly_once() {
        let lock = sengoo_async_rwlock_new_i64(37);
        let writer = unsafe { sengoo_async_rwlock_try_write_i64(lock) };
        assert!(writer > 0);

        let canceled_read = sengoo_async_rwlock_read_i64__start(lock);
        assert_eq!(
            unsafe { sengoo_async_rwlock_read_i64__poll(canceled_read) },
            0
        );
        assert_eq!(rwlock_waiter_counts(lock), (1, 0));
        assert!(unsafe { sengoo_async_rwlock_read_i64__cancel(canceled_read) });
        assert_eq!(rwlock_waiter_counts(lock), (0, 0));
        assert!(!unsafe { sengoo_async_rwlock_read_i64__cancel(0) });

        let dropped_write = sengoo_async_rwlock_write_i64__start(lock);
        assert_eq!(
            unsafe { sengoo_async_rwlock_write_i64__poll(dropped_write) },
            0
        );
        assert_eq!(rwlock_waiter_counts(lock), (0, 1));
        unsafe { sengoo_async_rwlock_write_i64__drop(dropped_write) };
        assert_eq!(rwlock_waiter_counts(lock), (0, 0));
        unsafe { sengoo_async_rwlock_write_i64__drop(0) };
        assert_eq!(rwlock_waiter_counts(lock), (0, 0));

        assert_eq!(
            unsafe { sengoo_async_rwlock_write_guard_unlock_i64(lock, writer) },
            0
        );
        unsafe { sengoo_async_rwlock_drop(lock) };
    }

    #[test]
    fn async_rwlock_close_wakes_pending_acquisitions_with_invalid_handle() {
        let lock = sengoo_async_rwlock_new_i64(41);
        let writer = unsafe { sengoo_async_rwlock_try_write_i64(lock) };
        assert!(writer > 0);

        let pending_read = sengoo_async_rwlock_read_i64__start(lock);
        let pending_write = sengoo_async_rwlock_write_i64__start(lock);
        assert_eq!(
            unsafe { sengoo_async_rwlock_read_i64__poll(pending_read) },
            0
        );
        assert_eq!(
            unsafe { sengoo_async_rwlock_write_i64__poll(pending_write) },
            0
        );
        assert_eq!(rwlock_waiter_counts(lock), (1, 1));

        unsafe { sengoo_async_rwlock_close(lock) };

        assert_eq!(
            unsafe { sengoo_async_rwlock_read_i64__poll(pending_read) },
            1
        );
        assert_eq!(
            unsafe { sengoo_async_rwlock_write_i64__poll(pending_write) },
            1
        );
        assert_eq!(
            unsafe { sengoo_async_rwlock_read_i64__result(pending_read) },
            -STATUS_INVALID_HANDLE
        );
        assert_eq!(
            unsafe { sengoo_async_rwlock_write_i64__result(pending_write) },
            -STATUS_INVALID_HANDLE
        );

        assert_eq!(
            unsafe { sengoo_async_rwlock_write_guard_unlock_i64(lock, writer) },
            0
        );
        unsafe { sengoo_async_rwlock_drop(lock) };
    }

    #[test]
    fn async_rwlock_i64_wrappers_and_invalid_poll_result_paths_stay_safe() {
        let lock = sengoo_async_rwlock_new_i64(53);

        let read = sengoo_async_rwlock_read_i64__start(lock);
        assert_eq!(unsafe { sengoo_async_rwlock_read_i64__poll(read) }, 1);
        assert_eq!(
            unsafe { sengoo_async_rwlock_read_i64__poll(read) },
            super::super::futures::POLL_ERROR_COMPLETED
        );
        assert_eq!(
            unsafe { sengoo_async_rwlock_read_i64__result(0) },
            -STATUS_INVALID_HANDLE
        );
        let read_guard = unsafe { sengoo_async_rwlock_read_i64__result(read) };
        assert!(read_guard > 0);
        assert_eq!(
            unsafe { sengoo_async_rwlock_read_guard_get_i64(lock, read_guard) },
            53
        );
        assert_eq!(
            unsafe { sengoo_async_rwlock_read_guard_unlock_i64(lock, read_guard) },
            0
        );
        assert_eq!(
            unsafe { sengoo_async_rwlock_read_guard_unlock_i64(lock, read_guard) },
            -STATUS_INVALID_HANDLE
        );

        let write = sengoo_async_rwlock_write_i64__start(lock);
        assert_eq!(unsafe { sengoo_async_rwlock_write_i64__poll(write) }, 1);
        let write_guard = unsafe { sengoo_async_rwlock_write_i64__result(write) };
        assert!(write_guard > 0);
        assert_eq!(
            unsafe { sengoo_async_rwlock_write_guard_set_i64(lock, write_guard, 59) },
            0
        );
        assert_eq!(
            unsafe { sengoo_async_rwlock_write_guard_get_i64(lock, write_guard) },
            59
        );
        assert_eq!(
            unsafe { sengoo_async_rwlock_write_guard_unlock_i64(lock, write_guard) },
            0
        );

        assert_eq!(unsafe { sengoo_async_rwlock_read_i64__poll(0) }, 1);
        assert_eq!(
            unsafe { sengoo_async_rwlock_write_i64__result(0) },
            -STATUS_INVALID_HANDLE
        );
        assert!(!unsafe { sengoo_async_rwlock_write_i64__cancel(0) });
        unsafe { sengoo_async_rwlock_read_i64__drop(0) };

        unsafe { sengoo_async_rwlock_drop(lock) };
    }

    #[test]
    fn generic_channel_queued_payload_drops_once_at_teardown() {
        let pair = sengoo_async_channel_bounded_parts(
            1,
            std::mem::size_of::<DropTrackedPayload>() as i64,
            std::mem::align_of::<DropTrackedPayload>() as i64,
            Some(drop_tracked_move),
            Some(drop_tracked_drop),
        );
        assert_ne!(pair, 0);
        let sender = unsafe { sengoo_async_channel_pair_sender(pair) };
        let receiver = unsafe { sengoo_async_channel_pair_receiver(pair) };

        let drop_counter = AtomicU32::new(0);
        let mut payload = drop_tracked_payload(41, &drop_counter);
        let send = unsafe {
            sengoo_async_channel_send__start(
                sender,
                (&mut payload as *mut DropTrackedPayload).cast::<c_void>(),
                Some(drop_tracked_drop),
            )
        };
        assert_ne!(send, 0);
        assert_eq!(unsafe { sengoo_async_channel_send__poll(send) }, 1);
        assert_eq!(unsafe { sengoo_async_channel_send__result(send) }, 0);
        assert_eq!(drop_counter.load(Ordering::SeqCst), 0);

        cleanup_channel_handles(pair, sender, receiver);
        assert_eq!(drop_counter.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn generic_channel_send_cancel_pending_closed_and_drop_paths_own_payload_exactly_once() {
        let drop_before_poll = AtomicU32::new(0);
        let pair_before_poll = sengoo_async_channel_bounded_parts(
            1,
            std::mem::size_of::<DropTrackedPayload>() as i64,
            std::mem::align_of::<DropTrackedPayload>() as i64,
            Some(drop_tracked_move),
            Some(drop_tracked_drop),
        );
        let sender_before_poll = unsafe { sengoo_async_channel_pair_sender(pair_before_poll) };
        let receiver_before_poll = unsafe { sengoo_async_channel_pair_receiver(pair_before_poll) };
        let mut before_poll = drop_tracked_payload(1, &drop_before_poll);
        let send_before_poll = unsafe {
            sengoo_async_channel_send__start(
                sender_before_poll,
                (&mut before_poll as *mut DropTrackedPayload).cast::<c_void>(),
                Some(drop_tracked_drop),
            )
        };
        assert_ne!(send_before_poll, 0);
        unsafe { sengoo_async_channel_send__drop(send_before_poll) };
        assert_eq!(drop_before_poll.load(Ordering::SeqCst), 1);
        cleanup_channel_handles(pair_before_poll, sender_before_poll, receiver_before_poll);

        let queued_drop = AtomicU32::new(0);
        let pending_drop = AtomicU32::new(0);
        let pair_pending = sengoo_async_channel_bounded_parts(
            1,
            std::mem::size_of::<DropTrackedPayload>() as i64,
            std::mem::align_of::<DropTrackedPayload>() as i64,
            Some(drop_tracked_move),
            Some(drop_tracked_drop),
        );
        let sender_pending = unsafe { sengoo_async_channel_pair_sender(pair_pending) };
        let receiver_pending = unsafe { sengoo_async_channel_pair_receiver(pair_pending) };

        let mut queued = drop_tracked_payload(2, &queued_drop);
        let queued_send = unsafe {
            sengoo_async_channel_send__start(
                sender_pending,
                (&mut queued as *mut DropTrackedPayload).cast::<c_void>(),
                Some(drop_tracked_drop),
            )
        };
        assert_eq!(unsafe { sengoo_async_channel_send__poll(queued_send) }, 1);
        assert_eq!(unsafe { sengoo_async_channel_send__result(queued_send) }, 0);
        assert_eq!(queued_drop.load(Ordering::SeqCst), 0);

        let mut pending = drop_tracked_payload(3, &pending_drop);
        let pending_send = unsafe {
            sengoo_async_channel_send__start(
                sender_pending,
                (&mut pending as *mut DropTrackedPayload).cast::<c_void>(),
                Some(drop_tracked_drop),
            )
        };
        assert_eq!(unsafe { sengoo_async_channel_send__poll(pending_send) }, 0);
        assert!(unsafe { sengoo_async_channel_send__cancel(pending_send) });
        assert_eq!(pending_drop.load(Ordering::SeqCst), 1);
        cleanup_channel_handles(pair_pending, sender_pending, receiver_pending);
        assert_eq!(queued_drop.load(Ordering::SeqCst), 1);

        let closed_drop = AtomicU32::new(0);
        let pair_closed = sengoo_async_channel_bounded_parts(
            1,
            std::mem::size_of::<DropTrackedPayload>() as i64,
            std::mem::align_of::<DropTrackedPayload>() as i64,
            Some(drop_tracked_move),
            Some(drop_tracked_drop),
        );
        let sender_closed = unsafe { sengoo_async_channel_pair_sender(pair_closed) };
        let receiver_closed = unsafe { sengoo_async_channel_pair_receiver(pair_closed) };
        let mut closed = drop_tracked_payload(4, &closed_drop);
        let closed_send = unsafe {
            sengoo_async_channel_send__start(
                sender_closed,
                (&mut closed as *mut DropTrackedPayload).cast::<c_void>(),
                Some(drop_tracked_drop),
            )
        };
        unsafe { sengoo_async_channel_sender_close(sender_closed) };
        assert_eq!(unsafe { sengoo_async_channel_send__poll(closed_send) }, 1);
        assert_eq!(
            unsafe { sengoo_async_channel_send__result(closed_send) },
            -STATUS_INVALID_HANDLE
        );
        assert_eq!(closed_drop.load(Ordering::SeqCst), 1);
        unsafe { sengoo_async_channel_receiver_drop(receiver_closed) };
        unsafe { drop(handle_take_box::<ChannelPair>(pair_closed)) };
    }

    #[test]
    fn generic_channel_received_payload_moves_out_once_and_replaces_initialized_output() {
        let pair = sengoo_async_channel_bounded_parts(
            1,
            std::mem::size_of::<DropTrackedPayload>() as i64,
            std::mem::align_of::<DropTrackedPayload>() as i64,
            Some(drop_tracked_move),
            Some(drop_tracked_drop),
        );
        let sender = unsafe { sengoo_async_channel_pair_sender(pair) };
        let receiver = unsafe { sengoo_async_channel_pair_receiver(pair) };

        let received_drop = AtomicU32::new(0);
        let replaced_drop = AtomicU32::new(0);
        let mut sent = drop_tracked_payload(7, &received_drop);
        let send = unsafe {
            sengoo_async_channel_send__start(
                sender,
                (&mut sent as *mut DropTrackedPayload).cast(),
                Some(drop_tracked_drop),
            )
        };
        assert_eq!(unsafe { sengoo_async_channel_send__poll(send) }, 1);
        assert_eq!(unsafe { sengoo_async_channel_send__result(send) }, 0);

        let recv = sengoo_async_channel_recv__start(receiver);
        assert_eq!(unsafe { sengoo_async_channel_recv__poll(recv) }, 1);
        let value_handle = unsafe { sengoo_async_channel_recv__result(recv) };
        assert!(value_handle > 0);

        let mut output = drop_tracked_payload(8, &replaced_drop);
        assert_eq!(
            unsafe {
                sengoo_async_channel_value_move_into(
                    value_handle,
                    (&mut output as *mut DropTrackedPayload).cast::<c_void>(),
                )
            },
            0
        );
        assert_eq!(replaced_drop.load(Ordering::SeqCst), 1);
        assert_eq!(received_drop.load(Ordering::SeqCst), 0);
        assert_eq!(output.tag, 7);

        unsafe { drop_tracked_drop((&mut output as *mut DropTrackedPayload).cast::<c_void>()) };
        assert_eq!(received_drop.load(Ordering::SeqCst), 1);
        cleanup_channel_handles(pair, sender, receiver);
    }

    #[test]
    fn generic_channel_value_move_into_consumes_handle_on_failure() {
        let pair = sengoo_async_channel_bounded_parts(
            1,
            std::mem::size_of::<DropTrackedPayload>() as i64,
            std::mem::align_of::<DropTrackedPayload>() as i64,
            Some(drop_tracked_move),
            Some(drop_tracked_drop),
        );
        let sender = unsafe { sengoo_async_channel_pair_sender(pair) };
        let receiver = unsafe { sengoo_async_channel_pair_receiver(pair) };

        let received_drop = AtomicU32::new(0);
        let mut sent = drop_tracked_payload(9, &received_drop);
        let send = unsafe {
            sengoo_async_channel_send__start(
                sender,
                (&mut sent as *mut DropTrackedPayload).cast(),
                Some(drop_tracked_drop),
            )
        };
        assert_eq!(unsafe { sengoo_async_channel_send__poll(send) }, 1);
        assert_eq!(unsafe { sengoo_async_channel_send__result(send) }, 0);

        let recv = sengoo_async_channel_recv__start(receiver);
        assert_eq!(unsafe { sengoo_async_channel_recv__poll(recv) }, 1);
        let value_handle = unsafe { sengoo_async_channel_recv__result(recv) };
        assert!(value_handle > 0);

        assert_eq!(
            unsafe { sengoo_async_channel_value_move_into(value_handle, std::ptr::null_mut()) },
            -STATUS_INVALID_ARGUMENT
        );
        assert_eq!(received_drop.load(Ordering::SeqCst), 1);

        cleanup_channel_handles(pair, sender, receiver);
    }

    #[test]
    fn generic_channel_enforces_backpressure_and_close_semantics() {
        let pair = sengoo_async_channel_bounded_parts(
            1,
            std::mem::size_of::<DropTrackedPayload>() as i64,
            std::mem::align_of::<DropTrackedPayload>() as i64,
            Some(drop_tracked_move),
            Some(drop_tracked_drop),
        );
        let sender = unsafe { sengoo_async_channel_pair_sender(pair) };
        let receiver = unsafe { sengoo_async_channel_pair_receiver(pair) };

        let first_drop = AtomicU32::new(0);
        let second_drop = AtomicU32::new(0);
        let mut first = drop_tracked_payload(11, &first_drop);
        let first_send = unsafe {
            sengoo_async_channel_send__start(
                sender,
                (&mut first as *mut DropTrackedPayload).cast(),
                Some(drop_tracked_drop),
            )
        };
        assert_eq!(unsafe { sengoo_async_channel_send__poll(first_send) }, 1);
        assert_eq!(unsafe { sengoo_async_channel_send__result(first_send) }, 0);

        let mut second = drop_tracked_payload(12, &second_drop);
        let second_send = unsafe {
            sengoo_async_channel_send__start(
                sender,
                (&mut second as *mut DropTrackedPayload).cast::<c_void>(),
                Some(drop_tracked_drop),
            )
        };
        assert_eq!(unsafe { sengoo_async_channel_send__poll(second_send) }, 0);

        let first_recv = sengoo_async_channel_recv__start(receiver);
        assert_eq!(unsafe { sengoo_async_channel_recv__poll(first_recv) }, 1);
        let first_value = unsafe { sengoo_async_channel_recv__result(first_recv) };
        assert!(first_value > 0);
        unsafe { sengoo_async_channel_value_drop(first_value) };
        assert_eq!(first_drop.load(Ordering::SeqCst), 1);

        assert_eq!(unsafe { sengoo_async_channel_send__poll(second_send) }, 1);
        assert_eq!(unsafe { sengoo_async_channel_send__result(second_send) }, 0);

        let second_recv = sengoo_async_channel_recv__start(receiver);
        assert_eq!(unsafe { sengoo_async_channel_recv__poll(second_recv) }, 1);
        let second_value = unsafe { sengoo_async_channel_recv__result(second_recv) };
        assert!(second_value > 0);
        unsafe { sengoo_async_channel_value_drop(second_value) };
        assert_eq!(second_drop.load(Ordering::SeqCst), 1);

        let closed_recv = sengoo_async_channel_recv__start(receiver);
        assert_eq!(unsafe { sengoo_async_channel_recv__poll(closed_recv) }, 0);
        unsafe { sengoo_async_channel_sender_close(sender) };
        assert_eq!(unsafe { sengoo_async_channel_recv__poll(closed_recv) }, 1);
        assert_eq!(
            unsafe { sengoo_async_channel_recv__result(closed_recv) },
            -STATUS_INVALID_HANDLE
        );

        unsafe { sengoo_async_channel_receiver_drop(receiver) };
        unsafe { drop(handle_take_box::<ChannelPair>(pair)) };
    }

    #[test]
    fn i64_channel_wrappers_stay_green() {
        let pair = sengoo_async_channel_bounded_i64(2);
        let sender = unsafe { sengoo_async_channel_pair_sender(pair) };
        let receiver = unsafe { sengoo_async_channel_pair_receiver(pair) };

        let send = sengoo_async_channel_send_i64__start(sender, 41);
        assert_eq!(unsafe { sengoo_async_channel_send_i64__poll(send) }, 1);
        let send_result = unsafe { sengoo_async_channel_send_i64__result(send) };
        assert!(send_result.is_ok);

        let recv = sengoo_async_channel_recv_i64__start(receiver);
        assert_eq!(unsafe { sengoo_async_channel_recv_i64__poll(recv) }, 1);
        let recv_result = unsafe { sengoo_async_channel_recv_i64__result(recv) };
        assert!(recv_result.is_ok);
        assert_eq!(recv_result.value, 41);

        let pending = sengoo_async_channel_recv_i64__start(receiver);
        assert_eq!(unsafe { sengoo_async_channel_recv_i64__poll(pending) }, 0);
        unsafe { sengoo_async_channel_sender_close(sender) };
        assert_eq!(unsafe { sengoo_async_channel_recv_i64__poll(pending) }, 1);
        let closed = unsafe { sengoo_async_channel_recv_i64__result(pending) };
        assert!(!closed.is_ok);
        assert_eq!(closed.error, STATUS_INVALID_HANDLE);

        unsafe { sengoo_async_channel_receiver_drop(receiver) };
        unsafe { drop(handle_take_box::<ChannelPair>(pair)) };
    }

    #[test]
    fn generic_channel_send_start_drops_payload_on_invalid_sender_and_zero_handle_stays_invalid() {
        let dropped = AtomicU32::new(0);
        let mut payload = drop_tracked_payload(13, &dropped);
        let send = unsafe {
            sengoo_async_channel_send__start(
                0,
                (&mut payload as *mut DropTrackedPayload).cast::<c_void>(),
                Some(drop_tracked_drop),
            )
        };
        assert_eq!(send, 0);
        assert_eq!(dropped.load(Ordering::SeqCst), 1);
        assert_eq!(unsafe { sengoo_async_channel_send__poll(send) }, 1);
        assert_eq!(
            unsafe { sengoo_async_channel_send__result(send) },
            -STATUS_INVALID_HANDLE
        );
        assert_eq!(dropped.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn channel_repeated_sender_extraction_is_one_shot_and_first_endpoint_stays_live() {
        let pair = sengoo_async_channel_bounded_parts(
            1,
            std::mem::size_of::<DropTrackedPayload>() as i64,
            std::mem::align_of::<DropTrackedPayload>() as i64,
            Some(drop_tracked_move),
            Some(drop_tracked_drop),
        );
        let sender = unsafe { sengoo_async_channel_pair_sender(pair) };
        assert!(sender > 0);
        assert_eq!(unsafe { sengoo_async_channel_pair_sender(pair) }, 0);
        let receiver = unsafe { sengoo_async_channel_pair_receiver(pair) };
        unsafe { sengoo_async_channel_pair_free(pair) };

        let payload_drop = AtomicU32::new(0);
        let mut payload = drop_tracked_payload(21, &payload_drop);
        let send = unsafe {
            sengoo_async_channel_send__start(
                sender,
                (&mut payload as *mut DropTrackedPayload).cast::<c_void>(),
                Some(drop_tracked_drop),
            )
        };
        assert_eq!(unsafe { sengoo_async_channel_send__poll(send) }, 1);
        assert_eq!(unsafe { sengoo_async_channel_send__result(send) }, 0);

        let recv = sengoo_async_channel_recv__start(receiver);
        assert_eq!(unsafe { sengoo_async_channel_recv__poll(recv) }, 1);
        let value = unsafe { sengoo_async_channel_recv__result(recv) };
        assert!(value > 0);
        unsafe { sengoo_async_channel_value_drop(value) };
        assert_eq!(payload_drop.load(Ordering::SeqCst), 1);

        let closed_recv = sengoo_async_channel_recv__start(receiver);
        assert_eq!(unsafe { sengoo_async_channel_recv__poll(closed_recv) }, 0);
        unsafe { sengoo_async_channel_sender_drop(sender) };
        assert_eq!(unsafe { sengoo_async_channel_recv__poll(closed_recv) }, 1);
        assert_eq!(
            unsafe { sengoo_async_channel_recv__result(closed_recv) },
            -STATUS_INVALID_HANDLE
        );
        unsafe { sengoo_async_channel_receiver_drop(receiver) };
    }

    #[test]
    fn channel_pair_drop_preserves_extracted_endpoints() {
        let pair = sengoo_async_channel_bounded_i64(2);
        let sender = unsafe { sengoo_async_channel_pair_sender(pair) };
        let receiver = unsafe { sengoo_async_channel_pair_receiver(pair) };
        unsafe { sengoo_async_channel_pair_free(pair) };

        let send = sengoo_async_channel_send_i64__start(sender, 55);
        assert_eq!(unsafe { sengoo_async_channel_send_i64__poll(send) }, 1);
        let send_result = unsafe { sengoo_async_channel_send_i64__result(send) };
        assert!(send_result.is_ok);

        let recv = sengoo_async_channel_recv_i64__start(receiver);
        assert_eq!(unsafe { sengoo_async_channel_recv_i64__poll(recv) }, 1);
        let result = unsafe { sengoo_async_channel_recv_i64__result(recv) };
        assert!(result.is_ok);
        assert_eq!(result.value, 55);

        let closed_recv = sengoo_async_channel_recv_i64__start(receiver);
        assert_eq!(
            unsafe { sengoo_async_channel_recv_i64__poll(closed_recv) },
            0
        );
        unsafe { sengoo_async_channel_sender_drop(sender) };
        assert_eq!(
            unsafe { sengoo_async_channel_recv_i64__poll(closed_recv) },
            1
        );
        let closed = unsafe { sengoo_async_channel_recv_i64__result(closed_recv) };
        assert!(!closed.is_ok);
        assert_eq!(closed.error, STATUS_INVALID_HANDLE);
        unsafe { sengoo_async_channel_receiver_drop(receiver) };
    }

    #[test]
    fn channel_repeated_receiver_extraction_is_one_shot_and_live_receiver_accepts_sends() {
        let pair = sengoo_async_channel_bounded_parts(
            1,
            std::mem::size_of::<DropTrackedPayload>() as i64,
            std::mem::align_of::<DropTrackedPayload>() as i64,
            Some(drop_tracked_move),
            Some(drop_tracked_drop),
        );
        let sender = unsafe { sengoo_async_channel_pair_sender(pair) };
        let receiver = unsafe { sengoo_async_channel_pair_receiver(pair) };
        assert!(receiver > 0);
        assert_eq!(unsafe { sengoo_async_channel_pair_receiver(pair) }, 0);
        unsafe { sengoo_async_channel_pair_free(pair) };

        let delivered_drop = AtomicU32::new(0);
        let mut delivered = drop_tracked_payload(31, &delivered_drop);
        let delivered_send = unsafe {
            sengoo_async_channel_send__start(
                sender,
                (&mut delivered as *mut DropTrackedPayload).cast::<c_void>(),
                Some(drop_tracked_drop),
            )
        };
        assert_eq!(
            unsafe { sengoo_async_channel_send__poll(delivered_send) },
            1
        );
        assert_eq!(
            unsafe { sengoo_async_channel_send__result(delivered_send) },
            0
        );

        let recv = sengoo_async_channel_recv__start(receiver);
        assert_eq!(unsafe { sengoo_async_channel_recv__poll(recv) }, 1);
        let value = unsafe { sengoo_async_channel_recv__result(recv) };
        assert!(value > 0);
        unsafe { sengoo_async_channel_value_drop(value) };
        assert_eq!(delivered_drop.load(Ordering::SeqCst), 1);

        unsafe { sengoo_async_channel_receiver_drop(receiver) };
        let rejected_drop = AtomicU32::new(0);
        let mut rejected = drop_tracked_payload(32, &rejected_drop);
        let rejected_send = unsafe {
            sengoo_async_channel_send__start(
                sender,
                (&mut rejected as *mut DropTrackedPayload).cast::<c_void>(),
                Some(drop_tracked_drop),
            )
        };
        assert_eq!(unsafe { sengoo_async_channel_send__poll(rejected_send) }, 1);
        assert_eq!(
            unsafe { sengoo_async_channel_send__result(rejected_send) },
            -STATUS_INVALID_HANDLE
        );
        assert_eq!(rejected_drop.load(Ordering::SeqCst), 1);
        unsafe { sengoo_async_channel_sender_drop(sender) };
    }

    #[test]
    fn channel_last_receiver_drop_discards_queue_and_fails_pending_and_new_sends_once() {
        let pair = sengoo_async_channel_bounded_parts(
            1,
            std::mem::size_of::<DropTrackedPayload>() as i64,
            std::mem::align_of::<DropTrackedPayload>() as i64,
            Some(drop_tracked_move),
            Some(drop_tracked_drop),
        );
        let sender = unsafe { sengoo_async_channel_pair_sender(pair) };
        let receiver = unsafe { sengoo_async_channel_pair_receiver(pair) };
        unsafe { sengoo_async_channel_pair_free(pair) };

        let queued_drop = AtomicU32::new(0);
        let mut queued = drop_tracked_payload(41, &queued_drop);
        let queued_send = unsafe {
            sengoo_async_channel_send__start(
                sender,
                (&mut queued as *mut DropTrackedPayload).cast::<c_void>(),
                Some(drop_tracked_drop),
            )
        };
        assert_eq!(unsafe { sengoo_async_channel_send__poll(queued_send) }, 1);
        assert_eq!(unsafe { sengoo_async_channel_send__result(queued_send) }, 0);

        let pending_drop = AtomicU32::new(0);
        let mut pending = drop_tracked_payload(42, &pending_drop);
        let pending_send = unsafe {
            sengoo_async_channel_send__start(
                sender,
                (&mut pending as *mut DropTrackedPayload).cast::<c_void>(),
                Some(drop_tracked_drop),
            )
        };
        assert_eq!(unsafe { sengoo_async_channel_send__poll(pending_send) }, 0);

        unsafe { sengoo_async_channel_receiver_drop(receiver) };
        assert_eq!(queued_drop.load(Ordering::SeqCst), 1);
        assert_eq!(pending_drop.load(Ordering::SeqCst), 0);
        assert_eq!(unsafe { sengoo_async_channel_send__poll(pending_send) }, 1);
        assert_eq!(
            unsafe { sengoo_async_channel_send__result(pending_send) },
            -STATUS_INVALID_HANDLE
        );
        assert_eq!(pending_drop.load(Ordering::SeqCst), 1);

        let new_drop = AtomicU32::new(0);
        let mut new_payload = drop_tracked_payload(43, &new_drop);
        let new_send = unsafe {
            sengoo_async_channel_send__start(
                sender,
                (&mut new_payload as *mut DropTrackedPayload).cast::<c_void>(),
                Some(drop_tracked_drop),
            )
        };
        assert_eq!(unsafe { sengoo_async_channel_send__poll(new_send) }, 1);
        assert_eq!(
            unsafe { sengoo_async_channel_send__result(new_send) },
            -STATUS_INVALID_HANDLE
        );
        assert_eq!(new_drop.load(Ordering::SeqCst), 1);
        unsafe { sengoo_async_channel_sender_drop(sender) };
    }
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
        export_addr!(sengoo_async_channel_bounded_parts),
        export_addr!(sengoo_async_channel_pair_sender),
        export_addr!(sengoo_async_channel_pair_receiver),
        export_addr!(sengoo_async_channel_pair_free),
        export_addr!(sengoo_async_channel_receiver_drop),
        export_addr!(sengoo_async_channel_sender_clone),
        export_addr!(sengoo_async_channel_sender_close),
        export_addr!(sengoo_async_channel_sender_drop),
        export_addr!(sengoo_async_channel_send__start),
        export_addr!(sengoo_async_channel_send__poll),
        export_addr!(sengoo_async_channel_send__result),
        export_addr!(sengoo_async_channel_send__cancel),
        export_addr!(sengoo_async_channel_send__drop),
        export_addr!(sengoo_async_channel_send_i64__start),
        export_addr!(sengoo_async_channel_send_i64__poll),
        export_addr!(sengoo_async_channel_send_i64__result),
        export_addr!(sengoo_async_channel_send_i64__cancel),
        export_addr!(sengoo_async_channel_send_i64__drop),
        export_addr!(sengoo_async_channel_recv__start),
        export_addr!(sengoo_async_channel_recv__poll),
        export_addr!(sengoo_async_channel_recv__result),
        export_addr!(sengoo_async_channel_recv__cancel),
        export_addr!(sengoo_async_channel_recv__drop),
        export_addr!(sengoo_async_channel_value_move_into),
        export_addr!(sengoo_async_channel_value_drop),
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
        export_addr!(sengoo_async_rwlock_new),
        export_addr!(sengoo_async_rwlock_new_parts),
        export_addr!(sengoo_async_rwlock_new_i64),
        export_addr!(sengoo_async_rwlock_close),
        export_addr!(sengoo_async_rwlock_drop),
        export_addr!(sengoo_async_rwlock_read__start),
        export_addr!(sengoo_async_rwlock_read__poll),
        export_addr!(sengoo_async_rwlock_read__result),
        export_addr!(sengoo_async_rwlock_read__cancel),
        export_addr!(sengoo_async_rwlock_read__drop),
        export_addr!(sengoo_async_rwlock_write__start),
        export_addr!(sengoo_async_rwlock_write__poll),
        export_addr!(sengoo_async_rwlock_write__result),
        export_addr!(sengoo_async_rwlock_write__cancel),
        export_addr!(sengoo_async_rwlock_write__drop),
        export_addr!(sengoo_async_rwlock_try_read),
        export_addr!(sengoo_async_rwlock_try_write),
        export_addr!(sengoo_async_rwlock_read_i64__start),
        export_addr!(sengoo_async_rwlock_read_i64__poll),
        export_addr!(sengoo_async_rwlock_read_i64__result),
        export_addr!(sengoo_async_rwlock_read_i64__cancel),
        export_addr!(sengoo_async_rwlock_read_i64__drop),
        export_addr!(sengoo_async_rwlock_write_i64__start),
        export_addr!(sengoo_async_rwlock_write_i64__poll),
        export_addr!(sengoo_async_rwlock_write_i64__result),
        export_addr!(sengoo_async_rwlock_write_i64__cancel),
        export_addr!(sengoo_async_rwlock_write_i64__drop),
        export_addr!(sengoo_async_rwlock_try_read_i64),
        export_addr!(sengoo_async_rwlock_try_write_i64),
        export_addr!(sengoo_async_rwlock_read_guard_copy_into),
        export_addr!(sengoo_async_rwlock_write_guard_copy_into),
        export_addr!(sengoo_async_rwlock_read_guard_get_i64),
        export_addr!(sengoo_async_rwlock_write_guard_get_i64),
        export_addr!(sengoo_async_rwlock_write_guard_set),
        export_addr!(sengoo_async_rwlock_write_guard_set_i64),
        export_addr!(sengoo_async_rwlock_read_guard_unlock),
        export_addr!(sengoo_async_rwlock_write_guard_unlock),
        export_addr!(sengoo_async_rwlock_read_guard_unlock_i64),
        export_addr!(sengoo_async_rwlock_write_guard_unlock_i64),
    ];
}
