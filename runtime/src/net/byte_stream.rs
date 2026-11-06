//! Generic bounded TCP/TLS byte workers. HTTP, SSE and application decisions
//! remain in Sengoo. No callback executes Sengoo code on these native threads.
use std::collections::{HashMap, VecDeque};
use std::io::{Read, Write};
use std::net::{Shutdown, TcpStream, ToSocketAddrs};
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::time::{Duration, Instant};

use super::{status_from_net_error_code, tls, TlsStream};

const MAX_WORKERS: usize = 8;
const MAX_REQUEST: usize = 1024 * 1024;
const MAX_EXPLICIT_REQUEST: usize = 16 * 1024 * 1024;
const MAX_REQUEST_COPIES: usize = 64 * 1024 * 1024;
const MIN_QUEUE: usize = 4096;
const MAX_QUEUE: usize = 1024 * 1024;
const PENDING: i64 = -100;
static NEXT: AtomicI64 = AtomicI64::new(1);
static WORKERS: AtomicUsize = AtomicUsize::new(0);
static REQUEST_COPY_BYTES: AtomicUsize = AtomicUsize::new(0);
static STREAMS: OnceLock<Mutex<HashMap<i64, Arc<State>>>> = OnceLock::new();
static DEADLINE_MONITOR: OnceLock<bool> = OnceLock::new();

// The reservation outlives the send. Releasing a handle or requesting cancel
// does not certify that its worker has stopped using the copied input.
struct RequestCopy {
    bytes: Vec<u8>,
    reserved: usize,
}
impl RequestCopy {
    unsafe fn capture(input: *const u8, length: usize) -> Result<Self, i64> {
        REQUEST_COPY_BYTES
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |used| {
                used.checked_add(length).filter(|next| *next <= MAX_REQUEST_COPIES)
            })
            .map_err(|_| 13i64)?;
        let mut copy = Self { bytes: Vec::new(), reserved: length };
        copy.bytes.try_reserve_exact(length).map_err(|_| 13i64)?;
        if length != 0 {
            copy.bytes.extend_from_slice(std::slice::from_raw_parts(input, length));
        }
        Ok(copy)
    }
}
impl Drop for RequestCopy {
    fn drop(&mut self) {
        // Free first: another admitted copy cannot overlap this allocation
        // after observing the returned reservation.
        drop(std::mem::take(&mut self.bytes));
        REQUEST_COPY_BYTES.fetch_sub(self.reserved, Ordering::AcqRel);
    }
}

#[derive(Default)]
struct Incoming {
    bytes: VecDeque<u8>,
    terminal: Option<i64>,
}
struct State {
    incoming: Mutex<Incoming>,
    space: Condvar,
    cancelled: AtomicBool,
    request_started: AtomicBool,
    stop_status: AtomicI64,
    connect_deadline: Mutex<Option<Instant>>,
    socket: Mutex<Option<TcpStream>>,
    capacity: usize,
}
fn streams() -> &'static Mutex<HashMap<i64, Arc<State>>> {
    STREAMS.get_or_init(|| Mutex::new(HashMap::new()))
}
fn state(handle: i64) -> Option<Arc<State>> {
    streams()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(&handle)
        .cloned()
}
fn finish(state: &State, status: i64) {
    let mut incoming = state.incoming.lock().unwrap_or_else(|e| e.into_inner());
    let stopped = state.stop_status.load(Ordering::Acquire);
    incoming.terminal = Some(if stopped != 0 { stopped } else { status });
    state.space.notify_all();
}
fn cancel(state: &State) {
    state.stop_status.store(19, Ordering::Release);
    state.cancelled.store(true, Ordering::Release);
    {
        let mut incoming = state.incoming.lock().unwrap_or_else(|e| e.into_inner());
        incoming.bytes.clear();
        incoming.terminal = Some(19);
        state.space.notify_all();
    }
    if let Some(socket) = state
        .socket
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .as_ref()
    {
        let _ = socket.shutdown(Shutdown::Both);
    }
}

fn ensure_deadline_monitor() -> bool {
    *DEADLINE_MONITOR.get_or_init(|| {
        std::thread::Builder::new()
            .name("sengoo-io-deadlines".into())
            .spawn(|| loop {
                std::thread::sleep(Duration::from_millis(10));
                let entries: Vec<_> = streams()
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .values()
                    .cloned()
                    .collect();
                for state in entries {
                    let deadline = state
                        .connect_deadline
                        .lock()
                        .unwrap_or_else(|e| e.into_inner());
                    if deadline.is_some_and(|deadline| Instant::now() >= deadline)
                        && state
                            .stop_status
                            .compare_exchange(0, 11, Ordering::AcqRel, Ordering::Acquire)
                            .is_ok()
                    {
                        state.cancelled.store(true, Ordering::Release);
                        state
                            .incoming
                            .lock()
                            .unwrap_or_else(|e| e.into_inner())
                            .terminal = Some(11);
                        if let Some(socket) = state
                            .socket
                            .lock()
                            .unwrap_or_else(|e| e.into_inner())
                            .as_ref()
                        {
                            let _ = socket.shutdown(Shutdown::Both);
                        }
                        state.space.notify_all();
                    }
                }
            })
            .is_ok()
    })
}

struct WorkerSlot;
impl Drop for WorkerSlot {
    fn drop(&mut self) {
        WORKERS.fetch_sub(1, Ordering::AcqRel);
    }
}

fn worker(
    state: Arc<State>,
    host: String,
    port: u16,
    secure: bool,
    request: RequestCopy,
    _connect_ms: u64,
    idle_ms: u64,
    root: Option<Vec<u8>>,
) {
    let _slot = WorkerSlot;
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> Result<(), i64> {
        let deadline = state
            .connect_deadline
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .unwrap();
        // OS name resolution has no portable interrupt API. It runs off the
        // caller thread and retains its worker slot until the resolver returns.
        let addresses: Vec<_> = (host.as_str(), port)
            .to_socket_addrs()
            .map_err(|_| 9i64)?
            .take(16)
            .collect();
        if addresses.is_empty() {
            return Err(9);
        }
        let mut connected = None;
        while connected.is_none() && Instant::now() < deadline {
            let mut saw_timeout = false;
            for address in &addresses {
                if state.cancelled.load(Ordering::Acquire) {
                    return Err(19);
                }
                let remaining = deadline.saturating_duration_since(Instant::now());
                if remaining.is_zero() {
                    break;
                }
                match TcpStream::connect_timeout(address, remaining.min(Duration::from_millis(100)))
                {
                    Ok(socket) => {
                        connected = Some(socket);
                        break;
                    }
                    Err(error)
                        if matches!(
                            error.kind(),
                            std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
                        ) =>
                    {
                        saw_timeout = true;
                    }
                    Err(_) => {}
                }
            }
            if connected.is_none() && !saw_timeout {
                return Err(9);
            }
        }
        let socket = connected.ok_or(11i64)?;
        socket.set_nodelay(true).map_err(|_| 9i64)?;
        *state.socket.lock().unwrap_or_else(|e| e.into_inner()) =
            Some(socket.try_clone().map_err(|_| 9i64)?);
        if state.cancelled.load(Ordering::Acquire) {
            return Err(19);
        }
        let handshake_timeout = deadline
            .saturating_duration_since(Instant::now())
            .max(Duration::from_millis(1));
        socket
            .set_read_timeout(Some(handshake_timeout))
            .map_err(|_| 9i64)?;
        socket
            .set_write_timeout(Some(handshake_timeout))
            .map_err(|_| 9i64)?;
        let mut stream = if secure {
            tls::connect_tls_with_root(socket, &host, root.as_deref())
                .map_err(|error| {
                    if Instant::now() >= deadline { 11 } else { status_from_net_error_code(error) }
                })?
        } else {
            TlsStream::Plain(socket)
        };
        {
            let mut active_deadline = state
                .connect_deadline
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            if Instant::now() >= deadline {
                return Err(11);
            }
            *active_deadline = None;
        }
        // Configure the socket held by TLS itself: cloned socket timeouts are
        // not shared consistently on every host. Clone shutdown interrupts I/O.
        stream
            .socket()
            .set_read_timeout(Some(Duration::from_millis(50)))
            .map_err(|_| 9i64)?;
        stream
            .socket()
            .set_write_timeout(Some(Duration::from_millis(idle_ms)))
            .map_err(|_| 9i64)?;
        let mut offset = 0;
        while offset < request.bytes.len() {
            if state.cancelled.load(Ordering::Acquire) {
                return Err(19);
            }
            // Conservative evidence: even a failed first write may have sent
            // bytes. DNS, TCP connection and TLS handshake do not set this.
            state.request_started.store(true, Ordering::Release);
            let written = stream.write(&request.bytes[offset..]).map_err(|error| {
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
                ) {
                    11i64
                } else {
                    9i64
                }
            })?;
            if written == 0 {
                return Err(9);
            }
            offset += written;
        }
        stream.flush().map_err(|_| 9i64)?;
        let mut last_data = Instant::now();
        let mut buffer = [0u8; 4096];
        loop {
            if state.cancelled.load(Ordering::Acquire) {
                return Err(19);
            }
            // Reserve room before reading so bounded queues also bound the
            // bytes held outside the queue. Every connection has its own lock.
            let mut incoming = state.incoming.lock().unwrap_or_else(|e| e.into_inner());
            while incoming.bytes.len() == state.capacity && !state.cancelled.load(Ordering::Acquire)
            {
                incoming = state
                    .space
                    .wait(incoming)
                    .unwrap_or_else(|e| e.into_inner());
                last_data = Instant::now(); // consumer backpressure is not network idleness
            }
            let available = (state.capacity - incoming.bytes.len()).min(buffer.len());
            drop(incoming);
            if state.cancelled.load(Ordering::Acquire) {
                return Err(19);
            }
            match stream.read(&mut buffer[..available]) {
                Ok(0) => return Ok(()),
                Ok(count) => {
                    last_data = Instant::now();
                    let mut incoming = state.incoming.lock().unwrap_or_else(|e| e.into_inner());
                    if state.cancelled.load(Ordering::Acquire) {
                        return Err(19);
                    }
                    incoming.bytes.extend(&buffer[..count]);
                }
                Err(error)
                    if matches!(
                        error.kind(),
                        std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
                    ) =>
                {
                    if last_data.elapsed() >= Duration::from_millis(idle_ms) {
                        return Err(11);
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
                Err(_) => return Err(9),
            }
        }
    }));
    finish(
        &state,
        match outcome {
            Ok(Ok(())) => 0,
            Ok(Err(status)) => status,
            Err(_) => 1,
        },
    );
    *state
        .connect_deadline
        .lock()
        .unwrap_or_else(|e| e.into_inner()) = None;
    state
        .socket
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .take();
    drop(request);
}

/// Returns a positive monotonically identified handle, or a negative stable
/// STATUS code. Inputs are copied before return. No DNS or I/O runs here.
/// # Safety
/// `host` points to `host_len` UTF-8 bytes and `request` to `request_len` bytes.
#[no_mangle]
pub unsafe extern "C" fn sengoo_net_stream_start(
    host: *const u8,
    host_len: i64,
    port: i64,
    secure: i64,
    request: *const u8,
    request_len: i64,
    connect_ms: i64,
    idle_ms: i64,
    capacity: i64,
) -> i64 {
    start(
        host,
        host_len,
        port,
        secure,
        request,
        request_len,
        connect_ms,
        idle_ms,
        capacity,
        None,
        MAX_REQUEST as i64,
    )
}

/// Same stream contract with one explicit DER CA, copied before returning.
/// # Safety
/// In addition to start's inputs, root points to root_len DER bytes.
#[no_mangle]
pub unsafe extern "C" fn sengoo_net_stream_start_ca(
    host: *const u8,
    host_len: i64,
    port: i64,
    request: *const u8,
    request_len: i64,
    connect_ms: i64,
    idle_ms: i64,
    capacity: i64,
    root: *const u8,
    root_len: i64,
) -> i64 {
    if root.is_null() || !(1..=65536).contains(&root_len) {
        return -2;
    }
    start(
        host,
        host_len,
        port,
        1,
        request,
        request_len,
        connect_ms,
        idle_ms,
        capacity,
        Some(std::slice::from_raw_parts(root, root_len as usize).to_vec()),
        MAX_REQUEST as i64,
    )
}

/// Explicit request limit, 1..=16 MiB; aggregate live copies are capped at 64 MiB.
/// Other limits and trust behavior are identical to sengoo_net_stream_start.
/// # Safety
/// The host and request pointers obey the same contract as the legacy start.
#[no_mangle]
pub unsafe extern "C" fn sengoo_net_stream_start_bounded(
    host: *const u8, host_len: i64, port: i64, secure: i64,
    request: *const u8, request_len: i64, connect_ms: i64, idle_ms: i64,
    capacity: i64, request_limit: i64,
) -> i64 {
    start(host, host_len, port, secure, request, request_len, connect_ms,
          idle_ms, capacity, None, request_limit)
}

/// Explicit-limit TLS stream with one per-connection DER trust anchor.
/// # Safety
/// In addition to start's inputs, root points to root_len DER bytes.
#[no_mangle]
pub unsafe extern "C" fn sengoo_net_stream_start_ca_bounded(
    host: *const u8, host_len: i64, port: i64,
    request: *const u8, request_len: i64, connect_ms: i64, idle_ms: i64,
    capacity: i64, root: *const u8, root_len: i64, request_limit: i64,
) -> i64 {
    if root.is_null() || !(1..=65536).contains(&root_len)
        || !(1..=MAX_EXPLICIT_REQUEST as i64).contains(&request_limit)
        || !(0..=request_limit).contains(&request_len)
        || (request.is_null() && request_len != 0) {
        return -2;
    }
    start(host, host_len, port, 1, request, request_len, connect_ms, idle_ms,
          capacity, Some(std::slice::from_raw_parts(root, root_len as usize).to_vec()),
          request_limit)
}

unsafe fn start(
    host: *const u8,
    host_len: i64,
    port: i64,
    secure: i64,
    request: *const u8,
    request_len: i64,
    connect_ms: i64,
    idle_ms: i64,
    capacity: i64,
    root: Option<Vec<u8>>,
    request_limit: i64,
) -> i64 {
    if host.is_null()
        || !(1..=253).contains(&host_len)
        || !(1..=65535).contains(&port)
        || !(0..=1).contains(&secure)
        || !(1..=MAX_EXPLICIT_REQUEST as i64).contains(&request_limit)
        || !(0..=request_limit).contains(&request_len)
        || (request.is_null() && request_len != 0)
        || !(1..=60_000).contains(&connect_ms)
        || !(1..=600_000).contains(&idle_ms)
        || !(MIN_QUEUE as i64..=MAX_QUEUE as i64).contains(&capacity)
    {
        return -2;
    }
    let host = match std::str::from_utf8(std::slice::from_raw_parts(host, host_len as usize)) {
        Ok(host) if !host.bytes().any(|c| c <= 32 || c == 127) => host.to_owned(),
        _ => return -2,
    };
    if !ensure_deadline_monitor() {
        return -9;
    }
    let mut table = streams().lock().unwrap_or_else(|e| e.into_inner());
    if table.len() >= MAX_WORKERS || WORKERS.load(Ordering::Acquire) >= MAX_WORKERS {
        return -13;
    }
    let request = match RequestCopy::capture(request, request_len as usize) {
        Ok(copy) => copy,
        Err(status) => return -status,
    };
    let handle = NEXT.fetch_add(1, Ordering::Relaxed);
    if handle <= 0 {
        return -13;
    }
    let state = Arc::new(State {
        incoming: Mutex::new(Incoming::default()),
        space: Condvar::new(),
        cancelled: AtomicBool::new(false),
        request_started: AtomicBool::new(false),
        stop_status: AtomicI64::new(0),
        connect_deadline: Mutex::new(Some(
            Instant::now() + Duration::from_millis(connect_ms as u64),
        )),
        socket: Mutex::new(None),
        capacity: capacity as usize,
    });
    table.insert(handle, state.clone());
    WORKERS.fetch_add(1, Ordering::AcqRel);
    if std::thread::Builder::new()
        .name("sengoo-byte-stream".into())
        .spawn(move || {
            worker(
                state,
                host,
                port as u16,
                secure != 0,
                request,
                connect_ms as u64,
                idle_ms as u64,
                root,
            )
        })
        .is_err()
    {
        table.remove(&handle);
        WORKERS.fetch_sub(1, Ordering::AcqRel);
        return -9;
    }
    handle
}

/// Positive=bytes, 0=EOF, -100=pending, other negative=stable STATUS.
/// Drains buffered data before a terminal network failure, preserving received
/// bytes for the Sengoo framing parser. Explicit cancellation discards them.
/// # Safety
/// `output` points to `capacity` writable bytes.
#[no_mangle]
pub unsafe extern "C" fn sengoo_net_stream_read(
    handle: i64,
    output: *mut u8,
    capacity: i64,
) -> i64 {
    if output.is_null() || capacity <= 0 || capacity > MAX_QUEUE as i64 {
        return -2;
    }
    let Some(state) = state(handle) else {
        return -3;
    };
    let mut incoming = state.incoming.lock().unwrap_or_else(|e| e.into_inner());
    let count = incoming.bytes.len().min(capacity as usize);
    for index in 0..count {
        *output.add(index) = incoming.bytes.pop_front().unwrap();
    }
    if count > 0 {
        state.space.notify_one();
        return count as i64;
    }
    match incoming.terminal {
        Some(status) => -status,
        None => PENDING,
    }
}

#[no_mangle]
pub extern "C" fn sengoo_net_stream_request_started(handle: i64) -> i64 {
    match state(handle) {
        Some(state) => i64::from(state.request_started.load(Ordering::Acquire)),
        None => -3,
    }
}
#[no_mangle]
pub extern "C" fn sengoo_net_stream_cancel(handle: i64) -> i64 {
    match state(handle) {
        Some(state) => {
            cancel(&state);
            1
        }
        None => 0,
    }
}
#[no_mangle]
pub extern "C" fn sengoo_net_stream_close(handle: i64) -> i64 {
    if let Some(state) = streams()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .remove(&handle)
    {
        cancel(&state);
    }
    0
}
#[no_mangle]
pub extern "C" fn sengoo_net_stream_live_workers() -> i64 {
    WORKERS.load(Ordering::Acquire) as i64
}
#[no_mangle]
pub extern "C" fn sengoo_net_stream_live_handles() -> i64 {
    streams().lock().unwrap_or_else(|e| e.into_inner()).len() as i64
}

#[no_mangle]
pub extern "C" fn sengoo_net_stream_live_request_bytes() -> i64 {
    REQUEST_COPY_BYTES.load(Ordering::Acquire) as i64
}

#[cfg(test)]
mod tests {
    use super::*;
    use rcgen::{BasicConstraints, CertificateParams, DnType, IsCa, KeyPair};
    use std::net::TcpListener;
    use std::thread::{self, JoinHandle};

    fn begin(port: u16, request: &[u8], idle: i64) -> i64 {
        unsafe {
            sengoo_net_stream_start(
                b"127.0.0.1".as_ptr(),
                9,
                port as i64,
                0,
                request.as_ptr(),
                request.len() as i64,
                2000,
                idle,
                4096,
            )
        }
    }
    fn collect(handle: i64) -> (Vec<u8>, i64) {
        let deadline = Instant::now() + Duration::from_secs(8);
        let mut bytes = Vec::new();
        let mut buffer = [0u8; 127];
        loop {
            let read =
                unsafe { sengoo_net_stream_read(handle, buffer.as_mut_ptr(), buffer.len() as i64) };
            if read > 0 {
                bytes.extend_from_slice(&buffer[..read as usize]);
            } else if read != PENDING {
                return (bytes, -read);
            }
            assert!(
                Instant::now() < deadline,
                "stream failed to reach terminal state"
            );
            if read == PENDING {
                thread::sleep(Duration::from_millis(2));
            }
        }
    }
    fn released() {
        let deadline = Instant::now() + Duration::from_secs(3);
        while WORKERS.load(Ordering::Acquire) != 0 {
            assert!(Instant::now() < deadline, "cancelled workers leaked");
            thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(sengoo_net_stream_live_handles(), 0);
        assert_eq!(sengoo_net_stream_live_request_bytes(), 0);
    }
    unsafe fn begin_bounded(port: u16, bytes: *const u8, length: i64, limit: i64) -> i64 {
        sengoo_net_stream_start_bounded(b"127.0.0.1".as_ptr(), 9, port as i64, 0,
            bytes, length, 2000, 3000, 4096, limit)
    }

    #[test]
    fn byte_stream_explicit_request_limits_validate_before_read_or_admission() {
        let _guard = crate::net::net_test_lock();
        let invalid = 1usize as *const u8;
        unsafe {
            assert_eq!(begin_bounded(1, invalid, MAX_REQUEST as i64 + 1, MAX_REQUEST as i64), -2);
            assert_eq!(begin_bounded(1, invalid, 1, 0), -2);
            assert_eq!(begin_bounded(1, invalid, 1, MAX_EXPLICIT_REQUEST as i64 + 1), -2);
            assert_eq!(begin_bounded(1, invalid, MAX_EXPLICIT_REQUEST as i64 + 1, MAX_EXPLICIT_REQUEST as i64), -2);
            assert_eq!(sengoo_net_stream_start(b"localhost".as_ptr(), 9, 1, 0, invalid,
                MAX_REQUEST as i64 + 1, 1000, 1000, 4096), -2);
            // An invalid size must not dereference either request or CA.
            assert_eq!(sengoo_net_stream_start_ca_bounded(b"localhost".as_ptr(), 9, 1,
                invalid, 1, 1000, 1000, 4096, invalid, 1, 0), -2);
        }
        released();
    }

    #[test]
    fn byte_stream_large_plain_request_is_an_independent_copy() {
        let _guard = crate::net::net_test_lock();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let mut input = vec![b'a'; MAX_REQUEST + 32768];
        input[33] = 0;
        let expected = input.clone();
        let (release, wait) = std::sync::mpsc::channel();
        let server = thread::spawn(move || {
            wait.recv_timeout(Duration::from_secs(5)).unwrap();
            let (mut socket, _) = listener.accept().unwrap();
            socket.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            let mut received = vec![0; expected.len()];
            socket.read_exact(&mut received).unwrap();
            assert_eq!(received, expected);
            socket.write_all(b"copied").unwrap();
        });
        let handle = unsafe { begin_bounded(port, input.as_ptr(), input.len() as i64, MAX_EXPLICIT_REQUEST as i64) };
        assert!(handle > 0);
        assert_eq!(sengoo_net_stream_live_request_bytes(), input.len() as i64);
        input.fill(b'z');
        drop(input);
        release.send(()).unwrap();
        assert_eq!(collect(handle), (b"copied".to_vec(), 0));
        assert!(sengoo_net_stream_request_started(handle) == 1);
        sengoo_net_stream_close(handle);
        server.join().unwrap();
        released();
    }

    #[test]
    fn byte_stream_large_copies_have_aggregate_admission_and_cancel_reclamation() {
        let _guard = crate::net::net_test_lock();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let (release, wait) = std::sync::mpsc::channel();
        let (accepted, ready) = std::sync::mpsc::channel();
        let server = thread::spawn(move || {
            let sockets: Vec<_> = (0..4).map(|_| listener.accept().unwrap().0).collect();
            accepted.send(()).unwrap();
            wait.recv_timeout(Duration::from_secs(10)).unwrap();
            drop(sockets);
        });
        let input = vec![b'x'; MAX_EXPLICIT_REQUEST];
        let handles: Vec<_> = (0..4).map(|_| unsafe {
            begin_bounded(port, input.as_ptr(), input.len() as i64, MAX_EXPLICIT_REQUEST as i64)
        }).collect();
        assert!(handles.iter().all(|handle| *handle > 0), "four admitted requests: {handles:?}");
        assert_eq!(sengoo_net_stream_live_request_bytes(), MAX_REQUEST_COPIES as i64);
        let before = sengoo_net_stream_live_handles();
        // A denied copy must not touch the caller's non-null invalid pointer.
        assert_eq!(unsafe { begin_bounded(port, 1usize as *const u8, 1, MAX_EXPLICIT_REQUEST as i64) }, -13);
        assert_eq!(sengoo_net_stream_live_handles(), before);
        assert_eq!(sengoo_net_stream_live_request_bytes(), MAX_REQUEST_COPIES as i64);
        ready.recv_timeout(Duration::from_secs(5)).unwrap();
        for handle in handles {
            sengoo_net_stream_cancel(handle);
            assert_eq!(collect(handle).1, 19);
            sengoo_net_stream_close(handle);
        }
        release.send(()).unwrap();
        server.join().unwrap();
        released();
        drop(input);
        let refused = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = refused.local_addr().unwrap().port();
        drop(refused);
        let input = vec![b'x'; MAX_REQUEST + 1];
        let handle = unsafe { begin_bounded(port, input.as_ptr(), input.len() as i64, MAX_EXPLICIT_REQUEST as i64) };
        assert!(handle > 0);
        assert_ne!(collect(handle).1, 0);
        assert_eq!(sengoo_net_stream_request_started(handle), 0);
        sengoo_net_stream_close(handle);
        released();
    }
    #[test]
    fn byte_stream_request_started_is_conservative_application_write_evidence() {
        let _guard = crate::net::net_test_lock();
        assert_eq!(sengoo_net_stream_request_started(0), -3);
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);
        let refused = begin(port, b"GET / HTTP/1.0\r\n\r\n", 1000);
        assert!(refused > 0);
        assert_ne!(collect(refused).1, 0);
        assert_eq!(sengoo_net_stream_request_started(refused), 0);
        sengoo_net_stream_close(refused);
        assert_eq!(sengoo_net_stream_request_started(refused), -3);
        released();

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            socket.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
            let mut bytes = [0u8; 128];
            assert!(socket.read(&mut bytes).unwrap() > 0);
            // A disconnect after sending must remain ambiguous to callers.
        });
        let sent = begin(port, b"POST / HTTP/1.0\r\n\r\n", 1000);
        assert!(sent > 0);
        collect(sent);
        server.join().unwrap();
        assert_eq!(sengoo_net_stream_request_started(sent), 1);
        sengoo_net_stream_cancel(sent);
        assert_eq!(sengoo_net_stream_request_started(sent), 1);
        sengoo_net_stream_close(sent);
        released();

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            socket.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
            let mut hello = [0u8; 4096];
            assert!(socket.read(&mut hello).unwrap() > 0);
            socket.write_all(b"not a TLS server").unwrap();
        });
        let handshake = unsafe { sengoo_net_stream_start(
            b"127.0.0.1".as_ptr(), 9, port as i64, 1,
            b"GET / HTTP/1.0\r\n\r\n".as_ptr(), 18, 2000, 1000, 4096,
        ) };
        assert!(handshake > 0);
        assert_ne!(collect(handshake).1, 0);
        server.join().unwrap();
        assert_eq!(sengoo_net_stream_request_started(handshake), 0);
        sengoo_net_stream_close(handshake);
        released();
    }
    fn read_request(stream: &mut impl Read) -> Vec<u8> {
        let mut request = Vec::new();
        let mut buffer = [0u8; 8192];
        loop {
            let count = stream.read(&mut buffer).expect("read request");
            assert!(count > 0);
            request.extend_from_slice(&buffer[..count]);
            if let Some(end) = request.windows(4).position(|bytes| bytes == b"\r\n\r\n") {
                let header = String::from_utf8_lossy(&request[..end]);
                let size: usize = header
                    .lines()
                    .find_map(|line| line.strip_prefix("Content-Length: "))
                    .unwrap_or("0")
                    .parse()
                    .unwrap();
                if request.len() >= end + 4 + size {
                    break;
                }
            }
            assert!(request.len() <= MAX_EXPLICIT_REQUEST);
        }
        request
    }

    #[test]
    fn byte_stream_plain_incremental_and_pending_eof_contract() {
        let _guard = crate::net::net_test_lock();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            assert_eq!(
                read_request(&mut socket),
                b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n"
            );
            for bytes in [
                b"HTTP/1.1 200 OK\r\n".as_slice(),
                b"Content-Length: 5\r\n\r\n",
                b"hello",
            ] {
                socket.write_all(bytes).unwrap();
                thread::sleep(Duration::from_millis(20));
            }
        });
        let now = Instant::now();
        let handle = begin(port, b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n", 2000);
        assert!(handle > 0 && now.elapsed() < Duration::from_millis(100));
        let mut byte = [0];
        assert_eq!(
            unsafe { sengoo_net_stream_read(handle, byte.as_mut_ptr(), 1) },
            PENDING
        );
        let (bytes, status) = collect(handle);
        assert_eq!(status, 0);
        assert!(bytes.ends_with(b"\r\n\r\nhello"));
        sengoo_net_stream_close(handle);
        sengoo_net_stream_close(handle);
        assert_eq!(
            unsafe { sengoo_net_stream_read(handle, byte.as_mut_ptr(), 1) },
            -3
        );
        server.join().unwrap();
        released();
    }

    #[test]
    fn byte_stream_bounds_backpressure_cancellation_and_idle_timeout() {
        let _guard = crate::net::net_test_lock();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            let _ = socket.write_all(&vec![b'x'; 128 * 1024]);
            thread::sleep(Duration::from_millis(200));
        });
        let handle = begin(port, b"", 1000);
        let deadline = Instant::now() + Duration::from_secs(2);
        while state(handle).unwrap().incoming.lock().unwrap().bytes.len() < 4096 {
            assert!(Instant::now() < deadline);
            thread::sleep(Duration::from_millis(5));
        }
        thread::sleep(Duration::from_millis(30));
        assert_eq!(
            state(handle).unwrap().incoming.lock().unwrap().bytes.len(),
            4096
        );
        assert_eq!(sengoo_net_stream_cancel(handle), 1);
        assert_eq!(collect(handle), (Vec::new(), 19));
        sengoo_net_stream_close(handle);
        server.join().unwrap();
        released();

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = thread::spawn(move || {
            let (_socket, _) = listener.accept().unwrap();
            thread::sleep(Duration::from_millis(250));
        });
        let handle = begin(port, b"", 70);
        assert_eq!(collect(handle), (Vec::new(), 11));
        sengoo_net_stream_close(handle);
        server.join().unwrap();
        released();
    }

    #[test]
    fn byte_stream_global_limit_and_cancel_during_handshake_release_slots() {
        let _guard = crate::net::net_test_lock();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = thread::spawn(move || {
            let mut sockets = Vec::new();
            for _ in 0..8 {
                sockets.push(listener.accept().unwrap().0);
            }
            thread::sleep(Duration::from_millis(200));
        });
        let mut handles = Vec::new();
        for _ in 0..8 {
            let handle = unsafe {
                sengoo_net_stream_start(
                    b"127.0.0.1".as_ptr(),
                    9,
                    port as i64,
                    1,
                    b"".as_ptr(),
                    0,
                    2000,
                    1000,
                    4096,
                )
            };
            assert!(handle > 0);
            handles.push(handle);
        }
        assert_eq!(begin(port, b"", 1000), -13);
        thread::sleep(Duration::from_millis(80));
        for handle in handles {
            sengoo_net_stream_cancel(handle);
            assert_eq!(collect(handle).1, 19);
            sengoo_net_stream_close(handle);
        }
        server.join().unwrap();
        released();
        assert_eq!(
            unsafe {
                sengoo_net_stream_start(
                    b"bad\nname".as_ptr(),
                    8,
                    port as i64,
                    0,
                    b"".as_ptr(),
                    0,
                    1,
                    1,
                    4096,
                )
            },
            -2
        );
    }

    #[test]
    fn byte_stream_handshake_deadline_is_visible_without_waiting_for_server_eof() {
        let _guard = crate::net::net_test_lock();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = thread::spawn(move || {
            let (_socket, _) = listener.accept().unwrap();
            thread::sleep(Duration::from_millis(350));
        });
        let handle = unsafe {
            sengoo_net_stream_start(
                b"127.0.0.1".as_ptr(),
                9,
                port as i64,
                1,
                b"".as_ptr(),
                0,
                70,
                1000,
                4096,
            )
        };
        let started = Instant::now();
        assert_eq!(collect(handle).1, 11);
        assert!(started.elapsed() < Duration::from_millis(300));
        sengoo_net_stream_close(handle);
        server.join().unwrap();
        released();
    }

    fn tls_fixture(host: &str, count: usize, response: Vec<u8>) -> (u16, Vec<u8>, JoinHandle<()>) {
        tls_fixture_expected(host, count, response, None)
    }

    fn tls_fixture_expected(host: &str, count: usize, response: Vec<u8>, expected: Option<Vec<u8>>) -> (u16, Vec<u8>, JoinHandle<()>) {
        let mut ca_params = CertificateParams::default();
        ca_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        ca_params
            .distinguished_name
            .push(DnType::CommonName, "Sengoo byte stream test root");
        let ca_key = KeyPair::generate().unwrap();
        let ca = ca_params.self_signed(&ca_key).unwrap();
        let key = KeyPair::generate().unwrap();
        let mut parameters = CertificateParams::new(vec![host.to_owned()]).unwrap();
        parameters.distinguished_name.push(DnType::CommonName, host);
        let cert = parameters.signed_by(&key, &ca, &ca_key).unwrap();
        let config = Arc::new(
            rustls::ServerConfig::builder()
                .with_no_client_auth()
                .with_single_cert(
                    vec![cert.der().clone()],
                    rustls::pki_types::PrivateKeyDer::Pkcs8(key.serialize_der().into()),
                )
                .unwrap(),
        );
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = thread::spawn(move || {
            let mut children = Vec::new();
            for _ in 0..count {
                let (socket, _) = listener.accept().unwrap();
                let config = config.clone();
                let response = response.clone();
                let expected = expected.clone();
                children.push(thread::spawn(move || {
                    socket
                        .set_read_timeout(Some(Duration::from_secs(3)))
                        .unwrap();
                    let connection = rustls::ServerConnection::new(config).unwrap();
                    let mut stream = rustls::StreamOwned::new(connection, socket);
                    let request =
                        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            read_request(&mut stream)
                        })) {
                            Ok(request) => request,
                            Err(_) => return,
                        };
                    if let Some(expected) = expected {
                        assert_eq!(request, expected);
                    } else {
                        assert!(request.starts_with(b"POST /messages HTTP/1.1\r\n"));
                        assert!(request
                            .windows(b"Authorization: Bearer fixture-only".len())
                            .any(|bytes| bytes == b"Authorization: Bearer fixture-only"));
                        assert!(request.ends_with("{\"text\":\"世界\"}".as_bytes()));
                    }
                    for (index, bytes) in response.chunks(97).enumerate() {
                        if stream.write_all(bytes).is_err() || stream.flush().is_err() {
                            return;
                        }
                        if index < 8 {
                            thread::sleep(Duration::from_millis(2));
                        }
                    }
                    stream.conn.send_close_notify();
                    let _ = stream.flush();
                }));
            }
            for child in children {
                child.join().unwrap();
            }
        });
        (port, ca.der().to_vec(), server)
    }
    fn begin_tls(port: u16, ca: &[u8]) -> i64 {
        let body = "{\"text\":\"世界\"}";
        let request = format!("POST /messages HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer fixture-only\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}", body.len(), body);
        unsafe {
            sengoo_net_stream_start_ca(
                b"localhost".as_ptr(),
                9,
                port as i64,
                request.as_ptr(),
                request.len() as i64,
                3000,
                2000,
                4096,
                ca.as_ptr(),
                ca.len() as i64,
            )
        }
    }

    #[test]
    fn byte_stream_large_tls_request_uses_the_explicit_limit_and_verified_ca() {
        let _guard = crate::net::net_test_lock();
        let body = "x".repeat(MAX_REQUEST + 32768) + "世界😀";
        let mut input = format!("POST /media HTTP/1.1\r\nHost: localhost\r\nContent-Length: {}\r\n\r\n{}", body.len(), body).into_bytes();
        let response = b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok".to_vec();
        let (port, ca, server) = tls_fixture_expected("localhost", 1, response.clone(), Some(input.clone()));
        let handle = unsafe { sengoo_net_stream_start_ca_bounded(
            b"localhost".as_ptr(), 9, port as i64, input.as_ptr(), input.len() as i64,
            3000, 3000, 4096, ca.as_ptr(), ca.len() as i64, MAX_EXPLICIT_REQUEST as i64,
        ) };
        assert!(handle > 0);
        input.fill(b'!');
        assert_eq!(collect(handle), (response, 0));
        assert_eq!(sengoo_net_stream_request_started(handle), 1);
        sengoo_net_stream_close(handle);
        server.join().unwrap();
        released();
    }

    #[test]
    fn byte_stream_four_verified_tls_requests_fragmented_with_independent_backpressure() {
        let _guard = crate::net::net_test_lock();
        let payload = "data: {\"delta\":\"世界😀\"}\r\n\r\n".repeat(700);
        let response = format!("HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nTransfer-Encoding: chunked\r\n\r\n{:x}\r\n{}\r\n0\r\n\r\n", payload.len(), payload).into_bytes();
        let (port, ca, server) = tls_fixture("localhost", 4, response.clone());
        let handles: Vec<_> = (0..4).map(|_| begin_tls(port, &ca)).collect();
        assert!(handles.iter().all(|handle| *handle > 0));
        // The first connection is deliberately not drained until the other
        // three complete, so a full queue cannot lock the connection registry.
        for index in [1, 2, 3, 0] {
            let (bytes, status) = collect(handles[index]);
            assert_eq!(status, 0, "TLS byte worker failed");
            assert_eq!(bytes, response, "fragmented raw bytes changed");
            sengoo_net_stream_close(handles[index]);
        }
        server.join().unwrap();
        released();
    }

    #[test]
    fn byte_stream_tls_rejects_wrong_hostname_and_untrusted_certificate() {
        let _guard = crate::net::net_test_lock();
        let (port, ca, server) = tls_fixture("wrong-host.invalid", 1, Vec::new());
        let handle = begin_tls(port, &ca);
        assert_eq!(collect(handle).1, 16);
        sengoo_net_stream_close(handle);
        server.join().unwrap();
        released();
        let (port, _ca, server) = tls_fixture("localhost", 1, Vec::new());
        let request = b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n";
        let handle = unsafe {
            sengoo_net_stream_start(
                b"localhost".as_ptr(),
                9,
                port as i64,
                1,
                request.as_ptr(),
                request.len() as i64,
                2000,
                1000,
                4096,
            )
        };
        assert_eq!(collect(handle).1, 15);
        sengoo_net_stream_close(handle);
        server.join().unwrap();
        released();
    }
}
