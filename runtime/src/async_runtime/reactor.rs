//! Reactor layer for timer, TCP socket, and owned-file-descriptor readiness.
//!
//! Interest registration bridges into the cooperative scheduler through the
//! existing poll wakeup hint thread-local.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use super::{record_poll_wakeup_hint, sengoo_async_poll_dispatch};

const STATUS_TIMEOUT: i64 = 11;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InterestKind {
    Timer,
    TcpReadable,
    OwnedFdReadable,
}

#[derive(Debug, Clone)]
struct Interest {
    kind: InterestKind,
    deadline: Option<Instant>,
    tcp_handle: Option<u64>,
    owned_fd: Option<i64>,
}

static REACTOR: OnceLock<Mutex<Reactor>> = OnceLock::new();

fn reactor() -> &'static Mutex<Reactor> {
    REACTOR.get_or_init(|| Mutex::new(Reactor::default()))
}

#[derive(Debug, Default)]
struct Reactor {
    next_id: u64,
    interests: HashMap<u64, Interest>,
}

impl Reactor {
    fn register(&mut self, interest: Interest) -> u64 {
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1).max(1);
        self.interests.insert(id, interest);
        id
    }

    fn unregister(&mut self, id: u64) -> bool {
        self.interests.remove(&id).is_some()
    }

    fn poll_interest(&self, id: u64) -> bool {
        let Some(interest) = self.interests.get(&id) else {
            return true;
        };

        match interest.kind {
            InterestKind::Timer => interest
                .deadline
                .is_some_and(|deadline| Instant::now() >= deadline),
            InterestKind::TcpReadable => interest.tcp_handle.is_some_and(tcp_socket_readable),
            InterestKind::OwnedFdReadable => interest.owned_fd.is_some_and(owned_fd_readable),
        }
    }

    fn interest_wakeup_deadline(&self, id: u64) -> Option<Instant> {
        let interest = self.interests.get(&id)?;
        match interest.kind {
            InterestKind::Timer => interest.deadline,
            InterestKind::TcpReadable | InterestKind::OwnedFdReadable => {
                Some(Instant::now() + Duration::from_millis(5))
            }
        }
    }
}

#[cfg(all(feature = "native-bridge", not(windows)))]
unsafe extern "C" {
    fn sengoo_tcp_poll_readable(handle: u64) -> i64;
}

fn tcp_socket_readable(handle: u64) -> bool {
    #[cfg(all(feature = "native-bridge", not(windows)))]
    {
        // When the native net bundle is linked, this resolves to a live poll helper.
        unsafe { sengoo_tcp_poll_readable(handle) != 0 }
    }
    #[cfg(all(feature = "native-bridge", windows))]
    {
        unsafe extern "system" {
            fn GetModuleHandleA(module_name: *const u8) -> isize;
            fn GetProcAddress(module: isize, symbol_name: *const u8) -> *mut core::ffi::c_void;
        }

        let module = unsafe { GetModuleHandleA(std::ptr::null()) };
        if module == 0 {
            return false;
        }
        let symbol = unsafe { GetProcAddress(module, c"sengoo_tcp_poll_readable".as_ptr().cast()) };
        if symbol.is_null() {
            return false;
        }
        let poll: unsafe extern "C" fn(u64) -> i64 = unsafe { std::mem::transmute(symbol) };
        unsafe { poll(handle) != 0 }
    }
    #[cfg(not(feature = "native-bridge"))]
    {
        let _ = handle;
        false
    }
}

#[cfg(unix)]
#[repr(C)]
struct PollFd {
    fd: i32,
    events: i16,
    revents: i16,
}

#[cfg(unix)]
unsafe extern "C" {
    fn poll(fds: *mut PollFd, nfds: usize, timeout: i32) -> i32;
}

#[cfg(windows)]
unsafe extern "C" {
    fn _get_osfhandle(fd: i32) -> isize;
    fn PeekNamedPipe(
        handle: isize,
        buffer: *mut core::ffi::c_void,
        buffer_size: u32,
        bytes_read: *mut u32,
        total_bytes_available: *mut u32,
        bytes_left_this_message: *mut u32,
    ) -> i32;
    fn GetFileType(handle: isize) -> u32;
    fn GetLastError() -> u32;
}

fn owned_fd_readable(fd: i64) -> bool {
    let Ok(fd) = i32::try_from(fd) else {
        return false;
    };
    if fd < 0 {
        return false;
    }

    #[cfg(unix)]
    {
        const POLLIN: i16 = 0x0001;
        const POLLERR: i16 = 0x0008;
        const POLLHUP: i16 = 0x0010;
        const POLLNVAL: i16 = 0x0020;
        let mut poll_fd = PollFd {
            fd,
            events: POLLIN,
            revents: 0,
        };
        let ready = unsafe { poll(&mut poll_fd, 1, 0) };
        return ready > 0
            && poll_fd.revents & (POLLIN | POLLERR | POLLHUP) != 0
            && poll_fd.revents & POLLNVAL == 0;
    }

    #[cfg(windows)]
    {
        const INVALID_HANDLE_VALUE: isize = -1;
        const FILE_TYPE_DISK: u32 = 0x0001;
        const FILE_TYPE_PIPE: u32 = 0x0003;
        const ERROR_BROKEN_PIPE: u32 = 109;

        let handle = unsafe { _get_osfhandle(fd) };
        if handle == INVALID_HANDLE_VALUE {
            return false;
        }
        match unsafe { GetFileType(handle) } {
            FILE_TYPE_DISK => true,
            FILE_TYPE_PIPE => {
                let mut available = 0u32;
                let ok = unsafe {
                    PeekNamedPipe(
                        handle,
                        std::ptr::null_mut(),
                        0,
                        std::ptr::null_mut(),
                        &mut available,
                        std::ptr::null_mut(),
                    )
                };
                ok != 0 && available > 0
                    || ok == 0 && unsafe { GetLastError() } == ERROR_BROKEN_PIPE
            }
            _ => false,
        }
    }

    #[cfg(not(any(unix, windows)))]
    {
        let _ = fd;
        false
    }
}

struct ReactorWaitState {
    interest_id: u64,
    child_kind: i64,
    child_handle: i64,
}

#[no_mangle]
pub extern "C" fn sengoo_async_reactor_timer_register(duration_ms: i64) -> u64 {
    let deadline = Instant::now() + Duration::from_millis(duration_ms.max(0) as u64);
    reactor()
        .lock()
        .expect("reactor mutex poisoned")
        .register(Interest {
            kind: InterestKind::Timer,
            deadline: Some(deadline),
            tcp_handle: None,
            owned_fd: None,
        })
}

#[no_mangle]
pub extern "C" fn sengoo_async_reactor_tcp_readable_register(tcp_handle: u64) -> u64 {
    reactor()
        .lock()
        .expect("reactor mutex poisoned")
        .register(Interest {
            kind: InterestKind::TcpReadable,
            deadline: None,
            tcp_handle: Some(tcp_handle),
            owned_fd: None,
        })
}

#[no_mangle]
pub extern "C" fn sengoo_async_reactor_fd_readable_register(owned_fd: i64) -> u64 {
    reactor()
        .lock()
        .expect("reactor mutex poisoned")
        .register(Interest {
            kind: InterestKind::OwnedFdReadable,
            deadline: None,
            tcp_handle: None,
            owned_fd: Some(owned_fd),
        })
}

#[no_mangle]
pub extern "C" fn sengoo_async_reactor_unregister(interest_id: u64) -> bool {
    reactor()
        .lock()
        .expect("reactor mutex poisoned")
        .unregister(interest_id)
}

#[no_mangle]
pub extern "C" fn sengoo_async_reactor_wait__start(
    interest_id: u64,
    child_kind: i64,
    child_handle: i64,
) -> i64 {
    let state = ReactorWaitState {
        interest_id,
        child_kind,
        child_handle,
    };
    Box::into_raw(Box::new(state)) as i64
}

#[no_mangle]
/// # Safety
///
/// `handle` must be zero or a live handle returned by
/// [`sengoo_async_reactor_wait__start`].
pub unsafe extern "C" fn sengoo_async_reactor_wait__poll(handle: i64) -> i64 {
    let Some(state) = super::handle_mut::<ReactorWaitState>(handle) else {
        return 1;
    };

    if sengoo_async_poll_dispatch(state.child_kind, state.child_handle) != 0 {
        return 1;
    }

    let reactor = reactor().lock().expect("reactor mutex poisoned");
    if reactor.poll_interest(state.interest_id) {
        return 1;
    }

    if let Some(deadline) = reactor.interest_wakeup_deadline(state.interest_id) {
        record_poll_wakeup_hint(deadline);
    }
    0
}

#[no_mangle]
/// # Safety
///
/// `handle` must be zero or an unconsumed handle returned by
/// [`sengoo_async_reactor_wait__start`].
pub unsafe extern "C" fn sengoo_async_reactor_wait__result(handle: i64) {
    let Some(state) = super::handle_take_box::<ReactorWaitState>(handle) else {
        return;
    };
    let _ = reactor()
        .lock()
        .expect("reactor mutex poisoned")
        .unregister(state.interest_id);
}

#[no_mangle]
/// # Safety
///
/// `handle` must be zero or an unconsumed handle returned by
/// [`sengoo_async_reactor_wait__start`].
pub unsafe extern "C" fn sengoo_async_reactor_wait__cancel(handle: i64) -> bool {
    let Some(state) = super::handle_take_box::<ReactorWaitState>(handle) else {
        return false;
    };
    let _ = reactor()
        .lock()
        .expect("reactor mutex poisoned")
        .unregister(state.interest_id);
    true
}

#[no_mangle]
/// # Safety
///
/// `handle` must be zero or an unconsumed handle returned by
/// [`sengoo_async_reactor_wait__start`].
pub unsafe extern "C" fn sengoo_async_reactor_wait__drop(handle: i64) {
    let Some(state) = super::handle_take_box::<ReactorWaitState>(handle) else {
        return;
    };
    let _ = reactor()
        .lock()
        .expect("reactor mutex poisoned")
        .unregister(state.interest_id);
}

pub(super) const STATUS_TIMEOUT_CODE: i64 = STATUS_TIMEOUT;
