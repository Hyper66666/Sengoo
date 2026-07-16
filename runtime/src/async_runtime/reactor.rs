//! Reactor layer for timer, TCP socket, and owned-file-descriptor readiness.
//!
//! Interest registration bridges into the cooperative scheduler through the
//! existing poll wakeup hint thread-local.

use std::collections::HashMap;
#[cfg(feature = "native-bridge")]
use std::fs::File;
use std::io::ErrorKind;
use std::net::{TcpListener, TcpStream};
#[cfg(all(feature = "native-bridge", unix))]
use std::os::fd::IntoRawFd;
#[cfg(all(feature = "native-bridge", windows))]
use std::os::windows::io::IntoRawHandle;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use super::record_poll_wakeup_hint;
#[cfg(feature = "native-bridge")]
use super::{sengoo_async_cancel_dispatch, sengoo_async_drop_dispatch, sengoo_async_poll_dispatch};

#[cfg(not(feature = "native-bridge"))]
unsafe fn sengoo_async_poll_dispatch(_kind: i64, _handle: i64) -> i64 {
    1
}

#[cfg(not(feature = "native-bridge"))]
unsafe fn sengoo_async_cancel_dispatch(_kind: i64, _handle: i64) -> bool {
    false
}

#[cfg(not(feature = "native-bridge"))]
unsafe fn sengoo_async_drop_dispatch(_kind: i64, _handle: i64) {}

#[cfg(feature = "native-bridge")]
const STATUS_TIMEOUT: i64 = 11;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InterestKind {
    Timer,
    TcpReadable,
    OwnedFdReadable,
    HttpListenerReadable,
}

#[derive(Debug)]
struct Interest {
    kind: InterestKind,
    deadline: Option<Instant>,
    tcp_handle: Option<u64>,
    owned_fd: Option<i64>,
    http_listener: Option<TcpListener>,
    accepted_stream: Option<TcpStream>,
    listener_error: Option<ErrorKind>,
}

impl Drop for Interest {
    fn drop(&mut self) {
        if self.kind != InterestKind::OwnedFdReadable {
            return;
        }
        let Some(handle) = self.owned_fd.take() else {
            return;
        };
        #[cfg(unix)]
        unsafe {
            close(handle as i32);
        }
        #[cfg(windows)]
        unsafe {
            CloseHandle(handle as isize);
        }
    }
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

    fn poll_interest(&mut self, id: u64) -> bool {
        let Some(interest) = self.interests.get_mut(&id) else {
            return true;
        };

        match interest.kind {
            InterestKind::Timer => interest
                .deadline
                .is_some_and(|deadline| Instant::now() >= deadline),
            InterestKind::TcpReadable => interest.tcp_handle.is_some_and(tcp_socket_readable),
            InterestKind::OwnedFdReadable => interest.owned_fd.is_some_and(owned_fd_readable),
            InterestKind::HttpListenerReadable => poll_http_listener_interest(interest),
        }
    }

    fn interest_wakeup_deadline(&self, id: u64) -> Option<Instant> {
        let interest = self.interests.get(&id)?;
        match interest.kind {
            InterestKind::Timer => interest.deadline,
            InterestKind::TcpReadable
            | InterestKind::OwnedFdReadable
            | InterestKind::HttpListenerReadable => Some(Instant::now() + Duration::from_millis(5)),
        }
    }

    fn take_http_listener_stream(&mut self, id: u64) -> Result<Option<TcpStream>, ErrorKind> {
        if !self.poll_interest(id) {
            return Ok(None);
        }
        let Some(interest) = self.interests.get_mut(&id) else {
            return Err(ErrorKind::NotFound);
        };
        if interest.kind != InterestKind::HttpListenerReadable {
            return Err(ErrorKind::InvalidInput);
        }
        if let Some(error) = interest.listener_error.take() {
            return Err(error);
        }
        Ok(interest.accepted_stream.take())
    }
}

fn poll_http_listener_interest(interest: &mut Interest) -> bool {
    if interest.accepted_stream.is_some() || interest.listener_error.is_some() {
        return true;
    }
    let Some(listener) = interest.http_listener.as_ref() else {
        interest.listener_error = Some(ErrorKind::NotFound);
        return true;
    };
    match listener.accept() {
        Ok((stream, _)) => match stream.set_nonblocking(false) {
            Ok(()) => interest.accepted_stream = Some(stream),
            Err(error) => interest.listener_error = Some(error.kind()),
        },
        Err(error) if error.kind() == ErrorKind::WouldBlock => return false,
        Err(error) => interest.listener_error = Some(error.kind()),
    }
    true
}

pub(crate) fn http_listener_register(listener: &TcpListener) -> Result<u64, ErrorKind> {
    let listener = listener.try_clone().map_err(|error| error.kind())?;
    listener
        .set_nonblocking(true)
        .map_err(|error| error.kind())?;
    let id = {
        let mut reactor = reactor().lock().expect("reactor mutex poisoned");
        reactor.register(Interest {
            kind: InterestKind::HttpListenerReadable,
            deadline: None,
            tcp_handle: None,
            owned_fd: None,
            http_listener: Some(listener),
            accepted_stream: None,
            listener_error: None,
        })
    };
    Ok(id)
}

pub(crate) fn http_listener_poll_accept(id: u64) -> Result<Option<TcpStream>, ErrorKind> {
    let mut reactor = reactor().lock().expect("reactor mutex poisoned");
    let result = reactor.take_http_listener_stream(id);
    if result.as_ref().is_ok_and(Option::is_none) {
        if let Some(deadline) = reactor.interest_wakeup_deadline(id) {
            record_poll_wakeup_hint(deadline);
        }
    }
    result
}

pub(crate) fn http_listener_unregister(id: u64) -> bool {
    reactor()
        .lock()
        .expect("reactor mutex poisoned")
        .unregister(id)
}

#[cfg(feature = "native-bridge")]
pub(crate) fn owned_file_register(file: &File) -> Result<u64, ErrorKind> {
    let duplicate = file.try_clone().map_err(|error| error.kind())?;
    #[cfg(unix)]
    let owned_fd = i64::from(duplicate.into_raw_fd());
    #[cfg(windows)]
    let owned_fd = duplicate.into_raw_handle() as isize as i64;
    #[cfg(not(any(unix, windows)))]
    {
        let _ = duplicate;
        return Err(ErrorKind::Unsupported);
    }

    let mut reactor = reactor().lock().expect("reactor mutex poisoned");
    Ok(reactor.register(Interest {
        kind: InterestKind::OwnedFdReadable,
        deadline: None,
        tcp_handle: None,
        owned_fd: Some(owned_fd),
        http_listener: None,
        accepted_stream: None,
        listener_error: None,
    }))
}

#[cfg(feature = "native-bridge")]
pub(crate) fn owned_file_poll_readable(id: u64) -> Result<bool, ErrorKind> {
    let mut reactor = reactor().lock().expect("reactor mutex poisoned");
    let Some(interest) = reactor.interests.get(&id) else {
        return Err(ErrorKind::NotFound);
    };
    if interest.kind != InterestKind::OwnedFdReadable {
        return Err(ErrorKind::InvalidInput);
    }
    let ready = reactor.poll_interest(id);
    if !ready {
        if let Some(deadline) = reactor.interest_wakeup_deadline(id) {
            record_poll_wakeup_hint(deadline);
        }
    }
    Ok(ready)
}

#[cfg(feature = "native-bridge")]
pub(crate) fn owned_file_unregister(id: u64) -> bool {
    reactor()
        .lock()
        .expect("reactor mutex poisoned")
        .unregister(id)
}

#[cfg(test)]
pub(crate) fn http_listener_interest_count() -> usize {
    let reactor = reactor().lock().expect("reactor mutex poisoned");
    reactor
        .interests
        .values()
        .filter(|interest| interest.kind == InterestKind::HttpListenerReadable)
        .count()
}

#[cfg(all(test, feature = "native-bridge"))]
pub(crate) fn interest_count() -> usize {
    reactor()
        .lock()
        .expect("reactor mutex poisoned")
        .interests
        .len()
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
            fn GetModuleHandleA(module_name: *const u8) -> *mut core::ffi::c_void;
            fn GetProcAddress(
                module: *mut core::ffi::c_void,
                symbol_name: *const u8,
            ) -> *mut core::ffi::c_void;
        }

        let module = unsafe { GetModuleHandleA(std::ptr::null()) };
        if module.is_null() {
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
    fn dup(fd: i32) -> i32;
    fn close(fd: i32) -> i32;
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
    fn GetCurrentProcess() -> isize;
    fn DuplicateHandle(
        source_process: isize,
        source_handle: isize,
        target_process: isize,
        target_handle: *mut isize,
        desired_access: u32,
        inherit_handle: i32,
        options: u32,
    ) -> i32;
    fn CloseHandle(handle: isize) -> i32;
}

fn owned_fd_readable(fd: i64) -> bool {
    #[cfg(unix)]
    {
        let Ok(fd) = i32::try_from(fd) else {
            return false;
        };
        if fd < 0 {
            return false;
        }
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

        let Ok(handle) = isize::try_from(fd) else {
            return false;
        };
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

fn registered_owned_fd(fd: i64) -> Option<i64> {
    let fd = i32::try_from(fd).ok().filter(|fd| *fd >= 0)?;
    #[cfg(windows)]
    {
        const INVALID_HANDLE_VALUE: isize = -1;
        const DUPLICATE_SAME_ACCESS: u32 = 0x0000_0002;
        let source = unsafe { _get_osfhandle(fd) };
        if source == INVALID_HANDLE_VALUE {
            return None;
        }
        let process = unsafe { GetCurrentProcess() };
        let mut duplicate = INVALID_HANDLE_VALUE;
        let duplicated = unsafe {
            DuplicateHandle(
                process,
                source,
                process,
                &mut duplicate,
                0,
                0,
                DUPLICATE_SAME_ACCESS,
            )
        };
        (duplicated != 0 && duplicate != INVALID_HANDLE_VALUE).then_some(duplicate as i64)
    }
    #[cfg(unix)]
    {
        let duplicate = unsafe { dup(fd) };
        (duplicate >= 0).then_some(i64::from(duplicate))
    }
    #[cfg(not(any(unix, windows)))]
    {
        None
    }
}

struct ReactorWaitState {
    interest_id: u64,
    child_kind: i64,
    child_handle: i64,
}

fn release_wait_state(state: ReactorWaitState) {
    let _ = reactor()
        .lock()
        .expect("reactor mutex poisoned")
        .unregister(state.interest_id);
    if state.child_handle == 0 {
        return;
    }
    let canceled = unsafe { sengoo_async_cancel_dispatch(state.child_kind, state.child_handle) };
    if !canceled {
        unsafe { sengoo_async_drop_dispatch(state.child_kind, state.child_handle) };
    }
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
            http_listener: None,
            accepted_stream: None,
            listener_error: None,
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
            http_listener: None,
            accepted_stream: None,
            listener_error: None,
        })
}

#[no_mangle]
pub extern "C" fn sengoo_async_reactor_fd_readable_register(owned_fd: i64) -> u64 {
    let owned_fd = registered_owned_fd(owned_fd);
    reactor()
        .lock()
        .expect("reactor mutex poisoned")
        .register(Interest {
            kind: InterestKind::OwnedFdReadable,
            deadline: None,
            tcp_handle: None,
            owned_fd,
            http_listener: None,
            accepted_stream: None,
            listener_error: None,
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

    let mut reactor = reactor().lock().expect("reactor mutex poisoned");
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
    release_wait_state(*state);
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
    release_wait_state(*state);
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
    release_wait_state(*state);
}

#[cfg(feature = "native-bridge")]
pub(super) const STATUS_TIMEOUT_CODE: i64 = STATUS_TIMEOUT;
