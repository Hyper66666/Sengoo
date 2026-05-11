use std::collections::HashMap;
use std::io::ErrorKind;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream, ToSocketAddrs, UdpSocket};
use std::sync::atomic::{AtomicI32, AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

static NET_RUNTIME: OnceLock<NetRuntime> = OnceLock::new();
static LAST_NET_ERROR: AtomicI32 = AtomicI32::new(SENGOO_NET_ERR_OK);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
enum NetErrorCode {
    Ok = 0,
    InvalidArgument = 1,
    InvalidUrl = 2,
    UnsupportedScheme = 3,
    ResolveFailed = 4,
    ConnectFailed = 5,
    IoError = 6,
    Timeout = 7,
    HttpProtocolError = 8,
    HttpChunkDecodeError = 9,
    WebSocketHandshakeError = 10,
    WebSocketProtocolError = 11,
    HandleNotFound = 12,
    InternalError = 13,
    RemoteClosed = 14,
}

pub const SENGOO_NET_ERR_OK: i32 = NetErrorCode::Ok as i32;
pub const SENGOO_NET_ERR_INVALID_ARGUMENT: i32 = NetErrorCode::InvalidArgument as i32;
pub const SENGOO_NET_ERR_INVALID_URL: i32 = NetErrorCode::InvalidUrl as i32;
pub const SENGOO_NET_ERR_UNSUPPORTED_SCHEME: i32 = NetErrorCode::UnsupportedScheme as i32;
pub const SENGOO_NET_ERR_RESOLVE_FAILED: i32 = NetErrorCode::ResolveFailed as i32;
pub const SENGOO_NET_ERR_CONNECT_FAILED: i32 = NetErrorCode::ConnectFailed as i32;
pub const SENGOO_NET_ERR_IO: i32 = NetErrorCode::IoError as i32;
pub const SENGOO_NET_ERR_TIMEOUT: i32 = NetErrorCode::Timeout as i32;
pub const SENGOO_NET_ERR_HTTP_PROTOCOL: i32 = NetErrorCode::HttpProtocolError as i32;
pub const SENGOO_NET_ERR_HTTP_CHUNKED: i32 = NetErrorCode::HttpChunkDecodeError as i32;
pub const SENGOO_NET_ERR_WS_HANDSHAKE: i32 = NetErrorCode::WebSocketHandshakeError as i32;
pub const SENGOO_NET_ERR_WS_PROTOCOL: i32 = NetErrorCode::WebSocketProtocolError as i32;
pub const SENGOO_NET_ERR_HANDLE_NOT_FOUND: i32 = NetErrorCode::HandleNotFound as i32;
pub const SENGOO_NET_ERR_INTERNAL: i32 = NetErrorCode::InternalError as i32;
pub const SENGOO_NET_ERR_REMOTE_CLOSED: i32 = NetErrorCode::RemoteClosed as i32;

fn set_last_error(code: NetErrorCode) {
    LAST_NET_ERROR.store(code as i32, Ordering::Relaxed);
}

fn reset_last_error() {
    set_last_error(NetErrorCode::Ok);
}

fn fail_handle(code: NetErrorCode) -> u64 {
    set_last_error(code);
    0
}

fn fail_i64(code: NetErrorCode) -> i64 {
    set_last_error(code);
    -1
}

fn fail_bool(code: NetErrorCode) -> i64 {
    set_last_error(code);
    0
}

fn classify_io_error(err: &std::io::Error) -> NetErrorCode {
    match err.kind() {
        ErrorKind::TimedOut | ErrorKind::WouldBlock => NetErrorCode::Timeout,
        _ => NetErrorCode::IoError,
    }
}

#[derive(Debug, Clone)]
struct ParsedUrl {
    scheme: String,
    host: String,
    port: u16,
    path: String,
}

#[derive(Debug, Clone)]
struct HttpResponseEntry {
    status_code: i64,
    body: Vec<u8>,
}

#[derive(Debug, Clone)]
struct HttpServerRequest {
    method: String,
    path: String,
    version: String,
    headers: HashMap<String, String>,
    body: Vec<u8>,
}

#[derive(Debug, Clone)]
struct HttpServerResponse {
    status: i32,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

#[derive(Debug, Clone)]
enum HttpServerRouteKind {
    StaticResponse { status: i32, body: Vec<u8> },
    WebSocketEcho,
}

#[derive(Debug, Clone)]
struct HttpServerRoute {
    method: String,
    path_pattern: String,
    kind: HttpServerRouteKind,
}

#[derive(Debug, Clone)]
enum HttpServerMiddlewareKind {
    RequireHeader {
        name: String,
        value: String,
        reject_status: i32,
        reject_body: Vec<u8>,
    },
}

#[derive(Debug, Clone)]
struct HttpServerMiddleware {
    kind: HttpServerMiddlewareKind,
}

#[derive(Debug)]
struct HttpServerState {
    listener: TcpListener,
    routes: Vec<HttpServerRoute>,
    middlewares: Vec<HttpServerMiddleware>,
    max_header_bytes: usize,
    max_body_bytes: usize,
}

#[derive(Debug)]
struct NetRuntime {
    next_handle: AtomicU64,
    tcp_streams: Mutex<HashMap<u64, TcpStream>>,
    udp_sockets: Mutex<HashMap<u64, UdpSocket>>,
    http_responses: Mutex<HashMap<u64, HttpResponseEntry>>,
    ws_streams: Mutex<HashMap<u64, TcpStream>>,
    http_servers: Mutex<HashMap<u64, HttpServerState>>,
}

enum RecvOutcome {
    Bytes(i64),
    Timeout,
}

impl NetRuntime {
    fn new() -> Self {
        Self {
            next_handle: AtomicU64::new(1),
            tcp_streams: Mutex::new(HashMap::new()),
            udp_sockets: Mutex::new(HashMap::new()),
            http_responses: Mutex::new(HashMap::new()),
            ws_streams: Mutex::new(HashMap::new()),
            http_servers: Mutex::new(HashMap::new()),
        }
    }

    #[allow(dead_code)]
    #[cfg(test)]
    fn reset_for_tests(&self) {
        self.next_handle.store(1, Ordering::Relaxed);
        if let Ok(mut table) = self.tcp_streams.lock() {
            table.clear();
        }
        if let Ok(mut table) = self.udp_sockets.lock() {
            table.clear();
        }
        if let Ok(mut table) = self.http_responses.lock() {
            table.clear();
        }
        if let Ok(mut table) = self.ws_streams.lock() {
            table.clear();
        }
        if let Ok(mut table) = self.http_servers.lock() {
            table.clear();
        }
        reset_last_error();
    }

    fn alloc_handle(&self) -> u64 {
        self.next_handle.fetch_add(1, Ordering::Relaxed)
    }

    fn tcp_connect(&self, host: &str, port: u16, timeout_ms: u32) -> Result<u64, NetErrorCode> {
        let addr = format!("{}:{}", host, port);
        let mut addrs = addr
            .to_socket_addrs()
            .map_err(|_| NetErrorCode::ResolveFailed)?;
        let first_addr = addrs.next().ok_or(NetErrorCode::ResolveFailed)?;
        let stream = TcpStream::connect_timeout(&first_addr, connect_timeout(timeout_ms)).map_err(
            |err| {
                if err.kind() == ErrorKind::TimedOut {
                    NetErrorCode::Timeout
                } else {
                    NetErrorCode::ConnectFailed
                }
            },
        )?;
        stream
            .set_nodelay(true)
            .map_err(|err| classify_io_error(&err))?;
        stream
            .set_read_timeout(Some(connect_timeout(timeout_ms)))
            .map_err(|err| classify_io_error(&err))?;
        stream
            .set_write_timeout(Some(connect_timeout(timeout_ms)))
            .map_err(|err| classify_io_error(&err))?;

        let handle = self.alloc_handle();
        self.tcp_streams
            .lock()
            .map_err(|_| NetErrorCode::InternalError)?
            .insert(handle, stream);
        Ok(handle)
    }

    fn tcp_send(&self, handle: u64, payload: &[u8]) -> Result<i64, NetErrorCode> {
        let mut table = self
            .tcp_streams
            .lock()
            .map_err(|_| NetErrorCode::InternalError)?;
        let stream = table.get_mut(&handle).ok_or(NetErrorCode::HandleNotFound)?;
        stream
            .write(payload)
            .map(|n| n as i64)
            .map_err(|err| classify_io_error(&err))
    }

    fn tcp_recv(
        &self,
        handle: u64,
        buffer: &mut [u8],
        timeout_ms: u32,
    ) -> Result<RecvOutcome, NetErrorCode> {
        let mut table = self
            .tcp_streams
            .lock()
            .map_err(|_| NetErrorCode::InternalError)?;
        let stream = table.get_mut(&handle).ok_or(NetErrorCode::HandleNotFound)?;

        if timeout_ms != 0 {
            stream
                .set_read_timeout(Some(Duration::from_millis(timeout_ms as u64)))
                .map_err(|err| classify_io_error(&err))?;
        }

        match stream.read(buffer) {
            Ok(n) => Ok(RecvOutcome::Bytes(n as i64)),
            Err(err) if err.kind() == ErrorKind::WouldBlock => Ok(RecvOutcome::Timeout),
            Err(err) if err.kind() == ErrorKind::TimedOut => Ok(RecvOutcome::Timeout),
            Err(err) => Err(classify_io_error(&err)),
        }
    }

    fn tcp_close(&self, handle: u64) -> Result<i64, NetErrorCode> {
        let removed = self
            .tcp_streams
            .lock()
            .map_err(|_| NetErrorCode::InternalError)?
            .remove(&handle)
            .is_some();
        if removed {
            Ok(1)
        } else {
            Err(NetErrorCode::HandleNotFound)
        }
    }

    fn udp_bind(&self, host: &str, port: u16) -> Result<u64, NetErrorCode> {
        let addr = format!("{}:{}", host, port);
        let socket = UdpSocket::bind(addr).map_err(|err| classify_io_error(&err))?;
        socket
            .set_nonblocking(false)
            .map_err(|err| classify_io_error(&err))?;

        let handle = self.alloc_handle();
        self.udp_sockets
            .lock()
            .map_err(|_| NetErrorCode::InternalError)?
            .insert(handle, socket);
        Ok(handle)
    }

    fn udp_connect(&self, handle: u64, addr: &str) -> Result<i64, NetErrorCode> {
        let mut table = self
            .udp_sockets
            .lock()
            .map_err(|_| NetErrorCode::InternalError)?;
        let socket = table.get_mut(&handle).ok_or(NetErrorCode::HandleNotFound)?;
        socket
            .connect(addr)
            .map(|_| 1)
            .map_err(|_| NetErrorCode::ConnectFailed)
    }

    fn udp_send(&self, handle: u64, payload: &[u8]) -> Result<i64, NetErrorCode> {
        let mut table = self
            .udp_sockets
            .lock()
            .map_err(|_| NetErrorCode::InternalError)?;
        let socket = table.get_mut(&handle).ok_or(NetErrorCode::HandleNotFound)?;
        socket
            .send(payload)
            .map(|n| n as i64)
            .map_err(|err| classify_io_error(&err))
    }

    fn udp_recv(
        &self,
        handle: u64,
        buffer: &mut [u8],
        timeout_ms: u32,
    ) -> Result<RecvOutcome, NetErrorCode> {
        let mut table = self
            .udp_sockets
            .lock()
            .map_err(|_| NetErrorCode::InternalError)?;
        let socket = table.get_mut(&handle).ok_or(NetErrorCode::HandleNotFound)?;
        if timeout_ms != 0 {
            socket
                .set_read_timeout(Some(Duration::from_millis(timeout_ms as u64)))
                .map_err(|err| classify_io_error(&err))?;
        }
        match socket.recv(buffer) {
            Ok(n) => Ok(RecvOutcome::Bytes(n as i64)),
            Err(err) if err.kind() == ErrorKind::WouldBlock => Ok(RecvOutcome::Timeout),
            Err(err) if err.kind() == ErrorKind::TimedOut => Ok(RecvOutcome::Timeout),
            Err(err) => Err(classify_io_error(&err)),
        }
    }

    fn udp_close(&self, handle: u64) -> Result<i64, NetErrorCode> {
        let removed = self
            .udp_sockets
            .lock()
            .map_err(|_| NetErrorCode::InternalError)?
            .remove(&handle)
            .is_some();
        if removed {
            Ok(1)
        } else {
            Err(NetErrorCode::HandleNotFound)
        }
    }

    fn http_store(&self, response: HttpResponseEntry) -> Result<u64, NetErrorCode> {
        let handle = self.alloc_handle();
        self.http_responses
            .lock()
            .map_err(|_| NetErrorCode::InternalError)?
            .insert(handle, response);
        Ok(handle)
    }

    fn http_status(&self, handle: u64) -> Result<i64, NetErrorCode> {
        let table = self
            .http_responses
            .lock()
            .map_err(|_| NetErrorCode::InternalError)?;
        table
            .get(&handle)
            .map(|resp| resp.status_code)
            .ok_or(NetErrorCode::HandleNotFound)
    }

    fn http_body_len(&self, handle: u64) -> Result<i64, NetErrorCode> {
        let table = self
            .http_responses
            .lock()
            .map_err(|_| NetErrorCode::InternalError)?;
        table
            .get(&handle)
            .map(|resp| resp.body.len() as i64)
            .ok_or(NetErrorCode::HandleNotFound)
    }

    fn http_body_copy(
        &self,
        handle: u64,
        buffer: *mut u8,
        capacity: usize,
    ) -> Result<i64, NetErrorCode> {
        let table = self
            .http_responses
            .lock()
            .map_err(|_| NetErrorCode::InternalError)?;
        let response = table.get(&handle).ok_or(NetErrorCode::HandleNotFound)?;
        Ok(copy_bytes_to_buffer(&response.body, buffer, capacity))
    }

    fn http_close(&self, handle: u64) -> Result<i64, NetErrorCode> {
        let removed = self
            .http_responses
            .lock()
            .map_err(|_| NetErrorCode::InternalError)?
            .remove(&handle)
            .is_some();
        if removed {
            Ok(1)
        } else {
            Err(NetErrorCode::HandleNotFound)
        }
    }

    fn ws_store(&self, stream: TcpStream) -> Result<u64, NetErrorCode> {
        let handle = self.alloc_handle();
        self.ws_streams
            .lock()
            .map_err(|_| NetErrorCode::InternalError)?
            .insert(handle, stream);
        Ok(handle)
    }

    fn ws_send_text(&self, handle: u64, payload: &[u8]) -> Result<i64, NetErrorCode> {
        let mut table = self
            .ws_streams
            .lock()
            .map_err(|_| NetErrorCode::InternalError)?;
        let stream = table.get_mut(&handle).ok_or(NetErrorCode::HandleNotFound)?;
        ws_write_frame(stream, 0x1, payload, true)
            .map(|_| payload.len() as i64)
            .map_err(|err| classify_io_error(&err))
    }

    fn ws_recv_text(
        &self,
        handle: u64,
        buffer: *mut u8,
        capacity: usize,
        timeout_ms: u32,
    ) -> Result<i64, NetErrorCode> {
        let mut table = self
            .ws_streams
            .lock()
            .map_err(|_| NetErrorCode::InternalError)?;
        let stream = table.get_mut(&handle).ok_or(NetErrorCode::HandleNotFound)?;
        if timeout_ms != 0 {
            stream
                .set_read_timeout(Some(Duration::from_millis(timeout_ms as u64)))
                .map_err(|err| classify_io_error(&err))?;
        }
        loop {
            let Some((opcode, payload)) = ws_read_frame(stream) else {
                return Err(NetErrorCode::WebSocketProtocolError);
            };
            match opcode {
                0x1 => return Ok(copy_bytes_to_buffer(&payload, buffer, capacity)),
                0x9 => {
                    ws_write_frame(stream, 0xA, &payload, true)
                        .map_err(|err| classify_io_error(&err))?;
                }
                0x8 => {
                    set_last_error(NetErrorCode::RemoteClosed);
                    return Ok(0);
                }
                0xA => {}
                _ => return Err(NetErrorCode::WebSocketProtocolError),
            }
        }
    }

    fn ws_close(&self, handle: u64) -> Result<i64, NetErrorCode> {
        let mut stream = self
            .ws_streams
            .lock()
            .map_err(|_| NetErrorCode::InternalError)?
            .remove(&handle)
            .ok_or(NetErrorCode::HandleNotFound)?;
        ws_write_frame(&mut stream, 0x8, &[], true).map_err(|err| classify_io_error(&err))?;
        Ok(1)
    }

    fn http_server_store(&self, state: HttpServerState) -> Result<u64, NetErrorCode> {
        let handle = self.alloc_handle();
        self.http_servers
            .lock()
            .map_err(|_| NetErrorCode::InternalError)?
            .insert(handle, state);
        Ok(handle)
    }

    fn http_server_local_port(&self, handle: u64) -> Result<i64, NetErrorCode> {
        let table = self
            .http_servers
            .lock()
            .map_err(|_| NetErrorCode::InternalError)?;
        let state = table.get(&handle).ok_or(NetErrorCode::HandleNotFound)?;
        state
            .listener
            .local_addr()
            .map(|addr| addr.port() as i64)
            .map_err(|err| classify_io_error(&err))
    }

    fn http_server_with_state<F, R>(&self, handle: u64, f: F) -> Result<R, NetErrorCode>
    where
        F: FnOnce(&mut HttpServerState) -> R,
    {
        let mut table = self
            .http_servers
            .lock()
            .map_err(|_| NetErrorCode::InternalError)?;
        let state = table.get_mut(&handle).ok_or(NetErrorCode::HandleNotFound)?;
        Ok(f(state))
    }

    fn http_server_set_limits(
        &self,
        handle: u64,
        max_header_bytes: usize,
        max_body_bytes: usize,
    ) -> Result<i64, NetErrorCode> {
        self.http_server_with_state(handle, |state| {
            state.max_header_bytes = max_header_bytes;
            state.max_body_bytes = max_body_bytes;
            1
        })
    }

    fn http_server_add_route(
        &self,
        handle: u64,
        route: HttpServerRoute,
    ) -> Result<i64, NetErrorCode> {
        self.http_server_with_state(handle, |state| {
            state.routes.push(route);
            1
        })
    }

    fn http_server_add_middleware(
        &self,
        handle: u64,
        middleware: HttpServerMiddleware,
    ) -> Result<i64, NetErrorCode> {
        self.http_server_with_state(handle, |state| {
            state.middlewares.push(middleware);
            1
        })
    }

    fn http_server_snapshot(
        &self,
        handle: u64,
    ) -> Result<
        (
            TcpListener,
            Vec<HttpServerRoute>,
            Vec<HttpServerMiddleware>,
            usize,
            usize,
        ),
        NetErrorCode,
    > {
        let table = self
            .http_servers
            .lock()
            .map_err(|_| NetErrorCode::InternalError)?;
        let state = table.get(&handle).ok_or(NetErrorCode::HandleNotFound)?;
        let listener = state
            .listener
            .try_clone()
            .map_err(|err| classify_io_error(&err))?;
        Ok((
            listener,
            state.routes.clone(),
            state.middlewares.clone(),
            state.max_header_bytes,
            state.max_body_bytes,
        ))
    }

    fn http_server_close(&self, handle: u64) -> Result<i64, NetErrorCode> {
        let removed = self
            .http_servers
            .lock()
            .map_err(|_| NetErrorCode::InternalError)?
            .remove(&handle)
            .is_some();
        if removed {
            Ok(1)
        } else {
            Err(NetErrorCode::HandleNotFound)
        }
    }
}

fn net_runtime() -> &'static NetRuntime {
    NET_RUNTIME.get_or_init(NetRuntime::new)
}

#[no_mangle]
pub extern "C" fn sengoo_net_last_error() -> i32 {
    LAST_NET_ERROR.load(Ordering::Relaxed)
}

#[no_mangle]
pub extern "C" fn sengoo_net_clear_error() {
    reset_last_error();
}

fn parse_host(host: *const u8) -> Result<String, NetErrorCode> {
    if host.is_null() {
        return Err(NetErrorCode::InvalidArgument);
    }
    unsafe {
        let mut len = 0usize;
        while *host.add(len) != 0 {
            len += 1;
            if len > 8192 {
                return Err(NetErrorCode::InvalidArgument);
            }
        }
        let bytes = std::slice::from_raw_parts(host, len);
        std::str::from_utf8(bytes)
            .map(str::to_string)
            .map_err(|_| NetErrorCode::InvalidArgument)
    }
}

fn parse_addr(host: *const u8, port: u16) -> Result<String, NetErrorCode> {
    let host = parse_host(host)?;
    Ok(format!("{}:{}", host, port))
}

fn parse_url(url: *const u8) -> Result<ParsedUrl, NetErrorCode> {
    let raw = parse_host(url)?;
    parse_url_str(&raw)
}

fn parse_url_str(raw: &str) -> Result<ParsedUrl, NetErrorCode> {
    let (scheme, rest) = raw.split_once("://").ok_or(NetErrorCode::InvalidUrl)?;
    let (host_port, path) = if let Some((h, p)) = rest.split_once('/') {
        (h, format!("/{}", p))
    } else {
        (rest, "/".to_string())
    };
    let (host, port) = if let Some((host, port)) = host_port.rsplit_once(':') {
        let port = port.parse::<u16>().map_err(|_| NetErrorCode::InvalidUrl)?;
        (host.to_string(), port)
    } else {
        let default_port =
            if scheme.eq_ignore_ascii_case("https") || scheme.eq_ignore_ascii_case("wss") {
                443
            } else {
                80
            };
        (host_port.to_string(), default_port)
    };

    if host.is_empty() {
        return Err(NetErrorCode::InvalidUrl);
    }

    Ok(ParsedUrl {
        scheme: scheme.to_ascii_lowercase(),
        host,
        port,
        path,
    })
}

fn connect_timeout(timeout_ms: u32) -> Duration {
    if timeout_ms == 0 {
        Duration::from_millis(5_000)
    } else {
        Duration::from_millis(timeout_ms as u64)
    }
}

fn copy_bytes_to_buffer(source: &[u8], buffer: *mut u8, capacity: usize) -> i64 {
    if buffer.is_null() {
        return -1;
    }
    let copy_len = source.len().min(capacity);
    unsafe {
        std::ptr::copy_nonoverlapping(source.as_ptr(), buffer, copy_len);
    }
    copy_len as i64
}

fn net_error_name(code: i32) -> &'static str {
    match code {
        SENGOO_NET_ERR_OK => "ok",
        SENGOO_NET_ERR_INVALID_ARGUMENT => "invalid_argument",
        SENGOO_NET_ERR_INVALID_URL => "invalid_url",
        SENGOO_NET_ERR_UNSUPPORTED_SCHEME => "unsupported_scheme",
        SENGOO_NET_ERR_RESOLVE_FAILED => "resolve_failed",
        SENGOO_NET_ERR_CONNECT_FAILED => "connect_failed",
        SENGOO_NET_ERR_IO => "io_error",
        SENGOO_NET_ERR_TIMEOUT => "timeout",
        SENGOO_NET_ERR_HTTP_PROTOCOL => "http_protocol_error",
        SENGOO_NET_ERR_HTTP_CHUNKED => "http_chunk_decode_error",
        SENGOO_NET_ERR_WS_HANDSHAKE => "websocket_handshake_error",
        SENGOO_NET_ERR_WS_PROTOCOL => "websocket_protocol_error",
        SENGOO_NET_ERR_HANDLE_NOT_FOUND => "handle_not_found",
        SENGOO_NET_ERR_INTERNAL => "internal_error",
        SENGOO_NET_ERR_REMOTE_CLOSED => "remote_closed",
        _ => "unknown_error",
    }
}

#[no_mangle]
pub extern "C" fn sengoo_net_error_name_copy(code: i32, buffer: *mut u8, capacity: usize) -> i64 {
    copy_bytes_to_buffer(net_error_name(code).as_bytes(), buffer, capacity)
}

fn open_stream(host: &str, port: u16, timeout_ms: u32) -> Result<TcpStream, NetErrorCode> {
    let addr = format!("{}:{}", host, port);
    let mut addrs = addr
        .to_socket_addrs()
        .map_err(|_| NetErrorCode::ResolveFailed)?;
    let first = addrs.next().ok_or(NetErrorCode::ResolveFailed)?;
    let stream =
        TcpStream::connect_timeout(&first, connect_timeout(timeout_ms)).map_err(|err| {
            if err.kind() == ErrorKind::TimedOut {
                NetErrorCode::Timeout
            } else {
                NetErrorCode::ConnectFailed
            }
        })?;
    let _ = stream.set_nodelay(true);
    let _ = stream.set_read_timeout(Some(connect_timeout(timeout_ms)));
    let _ = stream.set_write_timeout(Some(connect_timeout(timeout_ms)));
    Ok(stream)
}

fn parse_status_code(status_line: &str) -> Result<i64, NetErrorCode> {
    let mut parts = status_line.split_whitespace();
    let _http_version = parts.next().ok_or(NetErrorCode::HttpProtocolError)?;
    let status = parts
        .next()
        .ok_or(NetErrorCode::HttpProtocolError)?
        .parse::<i64>()
        .map_err(|_| NetErrorCode::HttpProtocolError)?;
    Ok(status)
}

fn split_http_headers_and_body(bytes: &[u8]) -> Result<(&[u8], &[u8]), NetErrorCode> {
    let header_end = bytes
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .ok_or(NetErrorCode::HttpProtocolError)?;
    Ok((&bytes[..header_end], &bytes[header_end + 4..]))
}

fn parse_http_headers(header_bytes: &[u8]) -> Result<(i64, HashMap<String, String>), NetErrorCode> {
    let header_text =
        std::str::from_utf8(header_bytes).map_err(|_| NetErrorCode::HttpProtocolError)?;
    let mut lines = header_text.split("\r\n");
    let status_line = lines.next().ok_or(NetErrorCode::HttpProtocolError)?;
    let status_code = parse_status_code(status_line)?;
    let mut headers = HashMap::new();
    for line in lines {
        if line.is_empty() {
            continue;
        }
        let (name, value) = line
            .split_once(':')
            .ok_or(NetErrorCode::HttpProtocolError)?;
        headers.insert(name.trim().to_ascii_lowercase(), value.trim().to_string());
    }
    Ok((status_code, headers))
}

fn decode_chunked_body(body: &[u8]) -> Result<Vec<u8>, NetErrorCode> {
    let mut idx = 0usize;
    let mut decoded = Vec::new();
    loop {
        let rel = body
            .get(idx..)
            .ok_or(NetErrorCode::HttpChunkDecodeError)?
            .windows(2)
            .position(|w| w == b"\r\n")
            .map(|p| p + idx)
            .ok_or(NetErrorCode::HttpChunkDecodeError)?;
        let size_line =
            std::str::from_utf8(&body[idx..rel]).map_err(|_| NetErrorCode::HttpChunkDecodeError)?;
        let size_hex = size_line
            .split(';')
            .next()
            .map(str::trim)
            .ok_or(NetErrorCode::HttpChunkDecodeError)?;
        let size =
            usize::from_str_radix(size_hex, 16).map_err(|_| NetErrorCode::HttpChunkDecodeError)?;
        idx = rel + 2;
        if size == 0 {
            if body.get(idx..).unwrap_or_default().starts_with(b"\r\n")
                || body.get(idx..).unwrap_or_default().is_empty()
            {
                return Ok(decoded);
            }
            return Err(NetErrorCode::HttpChunkDecodeError);
        }
        let end = idx
            .checked_add(size)
            .ok_or(NetErrorCode::HttpChunkDecodeError)?;
        decoded.extend_from_slice(
            body.get(idx..end)
                .ok_or(NetErrorCode::HttpChunkDecodeError)?,
        );
        let trailer = body
            .get(end..end + 2)
            .ok_or(NetErrorCode::HttpChunkDecodeError)?;
        if trailer != b"\r\n" {
            return Err(NetErrorCode::HttpChunkDecodeError);
        }
        idx = end + 2;
    }
}

fn send_http_request(
    method: &str,
    url: &ParsedUrl,
    body: &[u8],
    timeout_ms: u32,
) -> Result<HttpResponseEntry, NetErrorCode> {
    if url.scheme != "http" {
        return Err(NetErrorCode::UnsupportedScheme);
    }

    let mut stream = open_stream(&url.host, url.port, timeout_ms)?;
    let mut req = format!(
        "{} {} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\nUser-Agent: sengoo-runtime/0.1\r\n",
        method, url.path, url.host
    );
    if !body.is_empty() {
        req.push_str("Content-Type: application/octet-stream\r\n");
        req.push_str(&format!("Content-Length: {}\r\n", body.len()));
    }
    req.push_str("\r\n");

    stream
        .write_all(req.as_bytes())
        .map_err(|err| classify_io_error(&err))?;
    if !body.is_empty() {
        stream
            .write_all(body)
            .map_err(|err| classify_io_error(&err))?;
    }
    stream.flush().map_err(|err| classify_io_error(&err))?;

    let mut raw = Vec::new();
    stream
        .read_to_end(&mut raw)
        .map_err(|err| classify_io_error(&err))?;
    let (header, body_bytes) = split_http_headers_and_body(&raw)?;
    let (status_code, headers) = parse_http_headers(header)?;
    let body = if headers
        .get("transfer-encoding")
        .map(|v| v.to_ascii_lowercase().contains("chunked"))
        .unwrap_or(false)
    {
        decode_chunked_body(body_bytes)?
    } else {
        body_bytes.to_vec()
    };
    Ok(HttpResponseEntry { status_code, body })
}

fn read_c_buffer(ptr: *const u8, len: usize) -> Result<Vec<u8>, NetErrorCode> {
    if len == 0 {
        return Ok(Vec::new());
    }
    if ptr.is_null() {
        return Err(NetErrorCode::InvalidArgument);
    }
    let bytes = unsafe { std::slice::from_raw_parts(ptr, len) };
    Ok(bytes.to_vec())
}

fn http_reason_phrase(status: i32) -> &'static str {
    match status {
        101 => "Switching Protocols",
        200 => "OK",
        400 => "Bad Request",
        401 => "Unauthorized",
        404 => "Not Found",
        426 => "Upgrade Required",
        500 => "Internal Server Error",
        _ => "OK",
    }
}

fn parse_http_request_head(
    header_bytes: &[u8],
) -> Result<(String, String, String, HashMap<String, String>), NetErrorCode> {
    let header_text =
        std::str::from_utf8(header_bytes).map_err(|_| NetErrorCode::HttpProtocolError)?;
    let mut lines = header_text.split("\r\n");
    let request_line = lines.next().ok_or(NetErrorCode::HttpProtocolError)?;
    let mut parts = request_line.split_whitespace();
    let method = parts
        .next()
        .ok_or(NetErrorCode::HttpProtocolError)?
        .to_ascii_uppercase();
    let path = parts
        .next()
        .ok_or(NetErrorCode::HttpProtocolError)?
        .to_string();
    let version = parts
        .next()
        .ok_or(NetErrorCode::HttpProtocolError)?
        .to_string();
    if parts.next().is_some() || !path.starts_with('/') || !version.starts_with("HTTP/1.") {
        return Err(NetErrorCode::HttpProtocolError);
    }

    let mut headers = HashMap::new();
    for line in lines {
        if line.is_empty() {
            continue;
        }
        let (name, value) = line
            .split_once(':')
            .ok_or(NetErrorCode::HttpProtocolError)?;
        headers.insert(name.trim().to_ascii_lowercase(), value.trim().to_string());
    }

    Ok((method, path, version, headers))
}

fn read_http_request(
    stream: &mut TcpStream,
    max_header_bytes: usize,
    max_body_bytes: usize,
) -> Result<HttpServerRequest, NetErrorCode> {
    let mut raw = Vec::new();
    let mut buf = [0u8; 1024];
    let header_end = loop {
        let n = stream
            .read(&mut buf)
            .map_err(|err| classify_io_error(&err))?;
        if n == 0 {
            return Err(NetErrorCode::RemoteClosed);
        }
        raw.extend_from_slice(&buf[..n]);
        if raw.len()
            > max_header_bytes
                .saturating_add(max_body_bytes)
                .saturating_add(8192)
        {
            return Err(NetErrorCode::HttpProtocolError);
        }
        if let Some(pos) = raw.windows(4).position(|w| w == b"\r\n\r\n") {
            break pos;
        }
    };

    if header_end > max_header_bytes {
        return Err(NetErrorCode::HttpProtocolError);
    }

    let header_bytes = &raw[..header_end];
    let (method, path, version, headers) = parse_http_request_head(header_bytes)?;
    let mut body_bytes = raw[header_end + 4..].to_vec();

    let is_chunked = headers
        .get("transfer-encoding")
        .map(|v| v.to_ascii_lowercase().contains("chunked"))
        .unwrap_or(false);

    let body = if is_chunked {
        while !body_bytes.windows(5).any(|w| w == b"0\r\n\r\n") {
            let n = stream
                .read(&mut buf)
                .map_err(|err| classify_io_error(&err))?;
            if n == 0 {
                break;
            }
            body_bytes.extend_from_slice(&buf[..n]);
            if body_bytes.len() > max_body_bytes.saturating_add(4096) {
                return Err(NetErrorCode::HttpProtocolError);
            }
        }
        let decoded = decode_chunked_body(&body_bytes)?;
        if decoded.len() > max_body_bytes {
            return Err(NetErrorCode::HttpProtocolError);
        }
        decoded
    } else {
        let content_length = headers
            .get("content-length")
            .map(|value| {
                value
                    .trim()
                    .parse::<usize>()
                    .map_err(|_| NetErrorCode::HttpProtocolError)
            })
            .transpose()?
            .unwrap_or(0);
        if content_length > max_body_bytes {
            return Err(NetErrorCode::HttpProtocolError);
        }
        while body_bytes.len() < content_length {
            let n = stream
                .read(&mut buf)
                .map_err(|err| classify_io_error(&err))?;
            if n == 0 {
                return Err(NetErrorCode::HttpProtocolError);
            }
            body_bytes.extend_from_slice(&buf[..n]);
        }
        body_bytes[..content_length].to_vec()
    };

    Ok(HttpServerRequest {
        method,
        path,
        version,
        headers,
        body,
    })
}

fn write_http_response(
    stream: &mut TcpStream,
    response: &HttpServerResponse,
) -> Result<(), NetErrorCode> {
    let mut headers = response.headers.clone();
    if !headers
        .iter()
        .any(|(name, _)| name.eq_ignore_ascii_case("content-type"))
    {
        headers.push((
            "Content-Type".to_string(),
            "text/plain; charset=utf-8".to_string(),
        ));
    }
    if !headers
        .iter()
        .any(|(name, _)| name.eq_ignore_ascii_case("connection"))
    {
        headers.push(("Connection".to_string(), "close".to_string()));
    }
    if !headers
        .iter()
        .any(|(name, _)| name.eq_ignore_ascii_case("content-length"))
    {
        headers.push((
            "Content-Length".to_string(),
            response.body.len().to_string(),
        ));
    }

    let mut head = format!(
        "HTTP/1.1 {} {}\r\n",
        response.status,
        http_reason_phrase(response.status)
    );
    for (name, value) in headers {
        head.push_str(&format!("{}: {}\r\n", name, value));
    }
    head.push_str("\r\n");

    stream
        .write_all(head.as_bytes())
        .map_err(|err| classify_io_error(&err))?;
    stream
        .write_all(&response.body)
        .map_err(|err| classify_io_error(&err))?;
    stream.flush().map_err(|err| classify_io_error(&err))
}

fn split_path_segments(path: &str) -> Vec<&str> {
    path.trim_start_matches('/')
        .split('/')
        .filter(|seg| !seg.is_empty())
        .collect()
}

fn match_route_path(pattern: &str, path: &str) -> Option<HashMap<String, String>> {
    let lhs = split_path_segments(pattern);
    let rhs = split_path_segments(path);
    if lhs.len() != rhs.len() {
        return None;
    }

    let mut params = HashMap::new();
    for (pat, actual) in lhs.into_iter().zip(rhs.into_iter()) {
        if let Some(name) = pat.strip_prefix(':') {
            if name.is_empty() {
                return None;
            }
            params.insert(name.to_string(), actual.to_string());
            continue;
        }
        if pat != actual {
            return None;
        }
    }
    Some(params)
}

fn render_route_body(template: &[u8], params: &HashMap<String, String>) -> Vec<u8> {
    if params.is_empty() {
        return template.to_vec();
    }
    let Ok(mut text) = String::from_utf8(template.to_vec()) else {
        return template.to_vec();
    };
    for (key, value) in params {
        text = text.replace(&format!("{{{}}}", key), value);
    }
    text.into_bytes()
}

fn apply_middlewares(
    middlewares: &[HttpServerMiddleware],
    request: &HttpServerRequest,
) -> Option<HttpServerResponse> {
    for middleware in middlewares {
        match &middleware.kind {
            HttpServerMiddlewareKind::RequireHeader {
                name,
                value,
                reject_status,
                reject_body,
            } => {
                let actual = request.headers.get(name).map(String::as_str);
                if actual != Some(value.as_str()) {
                    return Some(HttpServerResponse {
                        status: *reject_status,
                        headers: Vec::new(),
                        body: reject_body.clone(),
                    });
                }
            }
        }
    }
    None
}

fn find_route(
    routes: &[HttpServerRoute],
    method: &str,
    path: &str,
) -> Option<(HttpServerRoute, HashMap<String, String>)> {
    for route in routes {
        if route.method != "*" && !route.method.eq_ignore_ascii_case(method) {
            continue;
        }
        if let Some(params) = match_route_path(&route.path_pattern, path) {
            return Some((route.clone(), params));
        }
    }
    None
}

fn base64_encode(input: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output = String::new();
    let mut idx = 0usize;
    while idx < input.len() {
        let b0 = input[idx];
        let b1 = if idx + 1 < input.len() {
            input[idx + 1]
        } else {
            0
        };
        let b2 = if idx + 2 < input.len() {
            input[idx + 2]
        } else {
            0
        };
        let triple = ((b0 as u32) << 16) | ((b1 as u32) << 8) | (b2 as u32);
        output.push(TABLE[((triple >> 18) & 0x3F) as usize] as char);
        output.push(TABLE[((triple >> 12) & 0x3F) as usize] as char);
        if idx + 1 < input.len() {
            output.push(TABLE[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            output.push('=');
        }
        if idx + 2 < input.len() {
            output.push(TABLE[(triple & 0x3F) as usize] as char);
        } else {
            output.push('=');
        }
        idx += 3;
    }
    output
}

fn sha1_digest(input: &[u8]) -> [u8; 20] {
    fn left_rotate(value: u32, bits: u32) -> u32 {
        value.rotate_left(bits)
    }

    let mut message = input.to_vec();
    let bit_len = (message.len() as u64) * 8;
    message.push(0x80);
    while (message.len() % 64) != 56 {
        message.push(0);
    }
    message.extend_from_slice(&bit_len.to_be_bytes());

    let mut h0: u32 = 0x67452301;
    let mut h1: u32 = 0xEFCDAB89;
    let mut h2: u32 = 0x98BADCFE;
    let mut h3: u32 = 0x10325476;
    let mut h4: u32 = 0xC3D2E1F0;

    for chunk in message.chunks(64) {
        let mut w = [0u32; 80];
        for (idx, word) in chunk.chunks(4).take(16).enumerate() {
            w[idx] = u32::from_be_bytes([word[0], word[1], word[2], word[3]]);
        }
        for idx in 16..80 {
            w[idx] = left_rotate(w[idx - 3] ^ w[idx - 8] ^ w[idx - 14] ^ w[idx - 16], 1);
        }

        let mut a = h0;
        let mut b = h1;
        let mut c = h2;
        let mut d = h3;
        let mut e = h4;

        for (idx, wi) in w.iter().enumerate() {
            let (f, k) = match idx {
                0..=19 => ((b & c) | ((!b) & d), 0x5A827999),
                20..=39 => (b ^ c ^ d, 0x6ED9EBA1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1BBCDC),
                _ => (b ^ c ^ d, 0xCA62C1D6),
            };
            let temp = left_rotate(a, 5)
                .wrapping_add(f)
                .wrapping_add(e)
                .wrapping_add(k)
                .wrapping_add(*wi);
            e = d;
            d = c;
            c = left_rotate(b, 30);
            b = a;
            a = temp;
        }

        h0 = h0.wrapping_add(a);
        h1 = h1.wrapping_add(b);
        h2 = h2.wrapping_add(c);
        h3 = h3.wrapping_add(d);
        h4 = h4.wrapping_add(e);
    }

    let mut out = [0u8; 20];
    out[0..4].copy_from_slice(&h0.to_be_bytes());
    out[4..8].copy_from_slice(&h1.to_be_bytes());
    out[8..12].copy_from_slice(&h2.to_be_bytes());
    out[12..16].copy_from_slice(&h3.to_be_bytes());
    out[16..20].copy_from_slice(&h4.to_be_bytes());
    out
}

fn websocket_accept_value(client_key: &str) -> String {
    let mut input = String::with_capacity(client_key.len() + 36);
    input.push_str(client_key.trim());
    input.push_str("258EAFA5-E914-47DA-95CA-C5AB0DC85B11");
    base64_encode(&sha1_digest(input.as_bytes()))
}

fn write_websocket_upgrade_response(
    stream: &mut TcpStream,
    accept: &str,
) -> Result<(), NetErrorCode> {
    let response = format!(
        "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {}\r\n\r\n",
        accept
    );
    stream
        .write_all(response.as_bytes())
        .map_err(|err| classify_io_error(&err))?;
    stream.flush().map_err(|err| classify_io_error(&err))
}

fn run_ws_echo_session(stream: &mut TcpStream) -> Result<(), NetErrorCode> {
    loop {
        let Some((opcode, payload)) = ws_read_frame(stream) else {
            return Err(NetErrorCode::RemoteClosed);
        };
        match opcode {
            0x1 => ws_write_frame(stream, 0x1, &payload, false)
                .map_err(|err| classify_io_error(&err))?,
            0x9 => ws_write_frame(stream, 0xA, &payload, false)
                .map_err(|err| classify_io_error(&err))?,
            0x8 => {
                let _ = ws_write_frame(stream, 0x8, &payload, false);
                return Ok(());
            }
            0xA => {}
            _ => return Err(NetErrorCode::WebSocketProtocolError),
        }
    }
}

fn process_http_server_connection(
    stream: &mut TcpStream,
    routes: &[HttpServerRoute],
    middlewares: &[HttpServerMiddleware],
    max_header_bytes: usize,
    max_body_bytes: usize,
) -> Result<(), NetErrorCode> {
    let request = match read_http_request(stream, max_header_bytes, max_body_bytes) {
        Ok(req) => req,
        Err(_) => {
            let _ = write_http_response(
                stream,
                &HttpServerResponse {
                    status: 400,
                    headers: Vec::new(),
                    body: b"bad request".to_vec(),
                },
            );
            return Ok(());
        }
    };
    let _request_http_version = request.version.as_str();
    let _request_body_len = request.body.len();

    if let Some(response) = apply_middlewares(middlewares, &request) {
        write_http_response(stream, &response)?;
        return Ok(());
    }

    let Some((route, params)) = find_route(routes, &request.method, &request.path) else {
        write_http_response(
            stream,
            &HttpServerResponse {
                status: 404,
                headers: Vec::new(),
                body: b"not found".to_vec(),
            },
        )?;
        return Ok(());
    };

    match route.kind {
        HttpServerRouteKind::StaticResponse { status, body } => {
            let rendered = render_route_body(&body, &params);
            write_http_response(
                stream,
                &HttpServerResponse {
                    status,
                    headers: Vec::new(),
                    body: rendered,
                },
            )?;
            Ok(())
        }
        HttpServerRouteKind::WebSocketEcho => {
            let is_upgrade = request.method.eq_ignore_ascii_case("GET")
                && request
                    .headers
                    .get("upgrade")
                    .map(|v| v.eq_ignore_ascii_case("websocket"))
                    .unwrap_or(false)
                && request
                    .headers
                    .get("connection")
                    .map(|v| v.to_ascii_lowercase().contains("upgrade"))
                    .unwrap_or(false)
                && request
                    .headers
                    .get("sec-websocket-version")
                    .map(|v| v.trim() == "13")
                    .unwrap_or(false);
            let Some(client_key) = request.headers.get("sec-websocket-key") else {
                write_http_response(
                    stream,
                    &HttpServerResponse {
                        status: 426,
                        headers: Vec::new(),
                        body: b"upgrade required".to_vec(),
                    },
                )?;
                return Ok(());
            };
            if !is_upgrade {
                write_http_response(
                    stream,
                    &HttpServerResponse {
                        status: 426,
                        headers: Vec::new(),
                        body: b"upgrade required".to_vec(),
                    },
                )?;
                return Ok(());
            }
            let accept = websocket_accept_value(client_key);
            write_websocket_upgrade_response(stream, &accept)?;
            run_ws_echo_session(stream)
        }
    }
}

fn accept_with_timeout(
    listener: &TcpListener,
    timeout_ms: u32,
) -> Result<Option<TcpStream>, NetErrorCode> {
    let deadline = Instant::now()
        .checked_add(Duration::from_millis(timeout_ms.max(1) as u64))
        .unwrap_or_else(Instant::now);
    loop {
        match listener.accept() {
            Ok((stream, _)) => return Ok(Some(stream)),
            Err(err) if err.kind() == ErrorKind::WouldBlock => {
                if Instant::now() >= deadline {
                    return Ok(None);
                }
                thread::sleep(Duration::from_millis(2));
            }
            Err(err) => return Err(classify_io_error(&err)),
        }
    }
}

fn websocket_client_key() -> &'static str {
    // Base64("0123456789abcdef")
    "MDEyMzQ1Njc4OWFiY2RlZg=="
}

fn read_http_response_headers(stream: &mut TcpStream) -> Result<Vec<u8>, NetErrorCode> {
    let mut bytes = Vec::new();
    let mut buf = [0u8; 1];
    while bytes.len() < 16 * 1024 {
        let n = stream
            .read(&mut buf)
            .map_err(|err| classify_io_error(&err))?;
        if n == 0 {
            break;
        }
        bytes.push(buf[0]);
        if bytes.ends_with(b"\r\n\r\n") {
            return Ok(bytes);
        }
    }
    Err(NetErrorCode::WebSocketHandshakeError)
}

fn websocket_connect(url: &ParsedUrl, timeout_ms: u32) -> Result<TcpStream, NetErrorCode> {
    if url.scheme != "ws" {
        return Err(NetErrorCode::UnsupportedScheme);
    }
    let mut stream = open_stream(&url.host, url.port, timeout_ms)?;
    let req = format!(
        "GET {} HTTP/1.1\r\nHost: {}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: {}\r\nSec-WebSocket-Version: 13\r\n\r\n",
        url.path,
        url.host,
        websocket_client_key()
    );
    stream
        .write_all(req.as_bytes())
        .map_err(|err| classify_io_error(&err))?;
    stream.flush().map_err(|err| classify_io_error(&err))?;

    let response = read_http_response_headers(&mut stream)?;
    let (header, _) = split_http_headers_and_body(&response)?;
    let (status_code, headers) = parse_http_headers(header)?;
    if status_code != 101 {
        return Err(NetErrorCode::WebSocketHandshakeError);
    }
    let upgrade_ok = headers
        .get("upgrade")
        .map(|v| v.eq_ignore_ascii_case("websocket"))
        .unwrap_or(false);
    let connection_ok = headers
        .get("connection")
        .map(|v| v.to_ascii_lowercase().contains("upgrade"))
        .unwrap_or(false);
    let accept_ok = headers
        .get("sec-websocket-accept")
        .map(|v| !v.trim().is_empty())
        .unwrap_or(false);
    if !upgrade_ok || !connection_ok || !accept_ok {
        return Err(NetErrorCode::WebSocketHandshakeError);
    }
    Ok(stream)
}

fn ws_write_frame(
    stream: &mut TcpStream,
    opcode: u8,
    payload: &[u8],
    masked: bool,
) -> std::io::Result<()> {
    let mut frame = Vec::with_capacity(payload.len() + 14);
    frame.push(0x80 | (opcode & 0x0F)); // FIN + opcode

    let mask_bit = if masked { 0x80 } else { 0x00 };
    if payload.len() <= 125 {
        frame.push(mask_bit | payload.len() as u8);
    } else if payload.len() <= 0xFFFF {
        frame.push(mask_bit | 126);
        frame.extend_from_slice(&(payload.len() as u16).to_be_bytes());
    } else {
        frame.push(mask_bit | 127);
        frame.extend_from_slice(&(payload.len() as u64).to_be_bytes());
    }

    if masked {
        let key = [0x13u8, 0x37, 0xC0, 0xDE];
        frame.extend_from_slice(&key);
        for (idx, byte) in payload.iter().enumerate() {
            frame.push(byte ^ key[idx % 4]);
        }
    } else {
        frame.extend_from_slice(payload);
    }

    stream.write_all(&frame)?;
    stream.flush()
}

fn ws_read_frame(stream: &mut TcpStream) -> Option<(u8, Vec<u8>)> {
    let mut header = [0u8; 2];
    stream.read_exact(&mut header).ok()?;
    let opcode = header[0] & 0x0F;
    let masked = (header[1] & 0x80) != 0;

    let mut payload_len = (header[1] & 0x7F) as usize;
    if payload_len == 126 {
        let mut ext = [0u8; 2];
        stream.read_exact(&mut ext).ok()?;
        payload_len = u16::from_be_bytes(ext) as usize;
    } else if payload_len == 127 {
        let mut ext = [0u8; 8];
        stream.read_exact(&mut ext).ok()?;
        payload_len = u64::from_be_bytes(ext) as usize;
    }

    let mut mask_key = [0u8; 4];
    if masked {
        stream.read_exact(&mut mask_key).ok()?;
    }

    let mut payload = vec![0u8; payload_len];
    if payload_len > 0 {
        stream.read_exact(&mut payload).ok()?;
    }

    if masked {
        for (idx, byte) in payload.iter_mut().enumerate() {
            *byte ^= mask_key[idx % 4];
        }
    }
    Some((opcode, payload))
}

#[no_mangle]
pub extern "C" fn sengoo_tcp_connect(host: *const u8, port: u16, timeout_ms: u32) -> u64 {
    reset_last_error();
    let host = match parse_host(host) {
        Ok(host) => host,
        Err(code) => return fail_handle(code),
    };
    net_runtime()
        .tcp_connect(&host, port, timeout_ms)
        .unwrap_or_else(fail_handle)
}

#[no_mangle]
/// # Safety
/// If `data` is non-null, it must point to `len` readable bytes.
pub unsafe extern "C" fn sengoo_tcp_send(handle: u64, data: *const u8, len: usize) -> i64 {
    reset_last_error();
    if data.is_null() {
        return fail_i64(NetErrorCode::InvalidArgument);
    }
    let payload = unsafe { std::slice::from_raw_parts(data, len) };
    net_runtime()
        .tcp_send(handle, payload)
        .unwrap_or_else(fail_i64)
}

#[no_mangle]
/// # Safety
/// If `buffer` is non-null, it must point to `capacity` writable bytes.
pub unsafe extern "C" fn sengoo_tcp_recv(
    handle: u64,
    buffer: *mut u8,
    capacity: usize,
    timeout_ms: u32,
) -> i64 {
    reset_last_error();
    if buffer.is_null() {
        return fail_i64(NetErrorCode::InvalidArgument);
    }
    let target = unsafe { std::slice::from_raw_parts_mut(buffer, capacity) };
    match net_runtime().tcp_recv(handle, target, timeout_ms) {
        Ok(RecvOutcome::Bytes(n)) => n,
        Ok(RecvOutcome::Timeout) => {
            set_last_error(NetErrorCode::Timeout);
            0
        }
        Err(code) => fail_i64(code),
    }
}

#[no_mangle]
pub extern "C" fn sengoo_tcp_close(handle: u64) -> i64 {
    reset_last_error();
    net_runtime().tcp_close(handle).unwrap_or_else(fail_bool)
}

#[no_mangle]
pub extern "C" fn sengoo_udp_bind(host: *const u8, port: u16) -> u64 {
    reset_last_error();
    let host = if host.is_null() {
        "0.0.0.0".to_string()
    } else {
        match parse_host(host) {
            Ok(host) => host,
            Err(code) => return fail_handle(code),
        }
    };
    net_runtime()
        .udp_bind(&host, port)
        .unwrap_or_else(fail_handle)
}

#[no_mangle]
pub extern "C" fn sengoo_udp_connect(handle: u64, host: *const u8, port: u16) -> i64 {
    reset_last_error();
    let addr = match parse_addr(host, port) {
        Ok(addr) => addr,
        Err(code) => return fail_bool(code),
    };
    net_runtime()
        .udp_connect(handle, &addr)
        .unwrap_or_else(fail_bool)
}

#[no_mangle]
/// # Safety
/// If `data` is non-null, it must point to `len` readable bytes.
pub unsafe extern "C" fn sengoo_udp_send(handle: u64, data: *const u8, len: usize) -> i64 {
    reset_last_error();
    if data.is_null() {
        return fail_i64(NetErrorCode::InvalidArgument);
    }
    let payload = unsafe { std::slice::from_raw_parts(data, len) };
    net_runtime()
        .udp_send(handle, payload)
        .unwrap_or_else(fail_i64)
}

#[no_mangle]
/// # Safety
/// If `buffer` is non-null, it must point to `capacity` writable bytes.
pub unsafe extern "C" fn sengoo_udp_recv(
    handle: u64,
    buffer: *mut u8,
    capacity: usize,
    timeout_ms: u32,
) -> i64 {
    reset_last_error();
    if buffer.is_null() {
        return fail_i64(NetErrorCode::InvalidArgument);
    }
    let target = unsafe { std::slice::from_raw_parts_mut(buffer, capacity) };
    match net_runtime().udp_recv(handle, target, timeout_ms) {
        Ok(RecvOutcome::Bytes(n)) => n,
        Ok(RecvOutcome::Timeout) => {
            set_last_error(NetErrorCode::Timeout);
            0
        }
        Err(code) => fail_i64(code),
    }
}

#[no_mangle]
pub extern "C" fn sengoo_udp_close(handle: u64) -> i64 {
    reset_last_error();
    net_runtime().udp_close(handle).unwrap_or_else(fail_bool)
}

#[no_mangle]
pub extern "C" fn sengoo_http_get(url: *const u8, timeout_ms: u32) -> u64 {
    reset_last_error();
    let parsed = match parse_url(url) {
        Ok(parsed) => parsed,
        Err(code) => return fail_handle(code),
    };
    let response = match send_http_request("GET", &parsed, &[], timeout_ms) {
        Ok(response) => response,
        Err(code) => return fail_handle(code),
    };
    net_runtime()
        .http_store(response)
        .unwrap_or_else(fail_handle)
}

#[no_mangle]
/// # Safety
/// If `body` is non-null, it must point to `len` readable bytes.
pub unsafe extern "C" fn sengoo_http_post(
    url: *const u8,
    body: *const u8,
    len: usize,
    timeout_ms: u32,
) -> u64 {
    reset_last_error();
    if body.is_null() && len > 0 {
        return fail_handle(NetErrorCode::InvalidArgument);
    }
    let parsed = match parse_url(url) {
        Ok(parsed) => parsed,
        Err(code) => return fail_handle(code),
    };
    let payload = if len == 0 {
        &[][..]
    } else {
        unsafe { std::slice::from_raw_parts(body, len) }
    };
    let response = match send_http_request("POST", &parsed, payload, timeout_ms) {
        Ok(response) => response,
        Err(code) => return fail_handle(code),
    };
    net_runtime()
        .http_store(response)
        .unwrap_or_else(fail_handle)
}

#[no_mangle]
pub extern "C" fn sengoo_http_status(handle: u64) -> i64 {
    reset_last_error();
    net_runtime().http_status(handle).unwrap_or_else(fail_i64)
}

#[no_mangle]
pub extern "C" fn sengoo_http_body_len(handle: u64) -> i64 {
    reset_last_error();
    net_runtime().http_body_len(handle).unwrap_or_else(fail_i64)
}

#[no_mangle]
pub extern "C" fn sengoo_http_body_copy(handle: u64, buffer: *mut u8, capacity: usize) -> i64 {
    reset_last_error();
    if buffer.is_null() {
        return fail_i64(NetErrorCode::InvalidArgument);
    }
    net_runtime()
        .http_body_copy(handle, buffer, capacity)
        .unwrap_or_else(fail_i64)
}

#[no_mangle]
pub extern "C" fn sengoo_http_close(handle: u64) -> i64 {
    reset_last_error();
    net_runtime().http_close(handle).unwrap_or_else(fail_bool)
}

#[no_mangle]
pub extern "C" fn sengoo_ws_connect(url: *const u8, timeout_ms: u32) -> u64 {
    reset_last_error();
    let parsed = match parse_url(url) {
        Ok(parsed) => parsed,
        Err(code) => return fail_handle(code),
    };
    let stream = match websocket_connect(&parsed, timeout_ms) {
        Ok(stream) => stream,
        Err(code) => return fail_handle(code),
    };
    net_runtime().ws_store(stream).unwrap_or_else(fail_handle)
}

#[no_mangle]
/// # Safety
/// If `data` is non-null, it must point to `len` readable bytes.
pub unsafe extern "C" fn sengoo_ws_send_text(handle: u64, data: *const u8, len: usize) -> i64 {
    reset_last_error();
    if data.is_null() {
        return fail_i64(NetErrorCode::InvalidArgument);
    }
    let payload = unsafe { std::slice::from_raw_parts(data, len) };
    net_runtime()
        .ws_send_text(handle, payload)
        .unwrap_or_else(fail_i64)
}

#[no_mangle]
pub extern "C" fn sengoo_ws_recv_text(
    handle: u64,
    buffer: *mut u8,
    capacity: usize,
    timeout_ms: u32,
) -> i64 {
    reset_last_error();
    if buffer.is_null() {
        return fail_i64(NetErrorCode::InvalidArgument);
    }
    net_runtime()
        .ws_recv_text(handle, buffer, capacity, timeout_ms)
        .unwrap_or_else(fail_i64)
}

#[no_mangle]
pub extern "C" fn sengoo_ws_close(handle: u64) -> i64 {
    reset_last_error();
    net_runtime().ws_close(handle).unwrap_or_else(fail_bool)
}

#[no_mangle]
pub extern "C" fn sengoo_http_server_bind(host: *const u8, port: u16) -> u64 {
    reset_last_error();
    let host = if host.is_null() {
        "127.0.0.1".to_string()
    } else {
        match parse_host(host) {
            Ok(host) => host,
            Err(code) => return fail_handle(code),
        }
    };
    let addr = format!("{}:{}", host, port);
    let listener = match TcpListener::bind(addr) {
        Ok(listener) => listener,
        Err(err) => return fail_handle(classify_io_error(&err)),
    };
    if let Err(err) = listener.set_nonblocking(true) {
        return fail_handle(classify_io_error(&err));
    }

    let state = HttpServerState {
        listener,
        routes: Vec::new(),
        middlewares: Vec::new(),
        max_header_bytes: 16 * 1024,
        max_body_bytes: 1024 * 1024,
    };
    net_runtime()
        .http_server_store(state)
        .unwrap_or_else(fail_handle)
}

#[no_mangle]
pub extern "C" fn sengoo_http_server_local_port(handle: u64) -> i64 {
    reset_last_error();
    net_runtime()
        .http_server_local_port(handle)
        .unwrap_or_else(fail_i64)
}

#[no_mangle]
pub extern "C" fn sengoo_http_server_set_limits(
    handle: u64,
    max_header_bytes: u32,
    max_body_bytes: u32,
) -> i64 {
    reset_last_error();
    if max_header_bytes == 0 || max_body_bytes == 0 {
        return fail_bool(NetErrorCode::InvalidArgument);
    }
    net_runtime()
        .http_server_set_limits(handle, max_header_bytes as usize, max_body_bytes as usize)
        .unwrap_or_else(fail_bool)
}

#[no_mangle]
pub extern "C" fn sengoo_http_server_add_route(
    handle: u64,
    method: *const u8,
    path_pattern: *const u8,
    status: i32,
    body: *const u8,
    body_len: usize,
) -> i64 {
    reset_last_error();
    if !(100..=599).contains(&status) {
        return fail_bool(NetErrorCode::InvalidArgument);
    }
    let method = match parse_host(method) {
        Ok(method) => method.to_ascii_uppercase(),
        Err(code) => return fail_bool(code),
    };
    let path_pattern = match parse_host(path_pattern) {
        Ok(path) if path.starts_with('/') => path,
        Ok(_) => return fail_bool(NetErrorCode::InvalidArgument),
        Err(code) => return fail_bool(code),
    };
    let body = match read_c_buffer(body, body_len) {
        Ok(body) => body,
        Err(code) => return fail_bool(code),
    };

    let route = HttpServerRoute {
        method,
        path_pattern,
        kind: HttpServerRouteKind::StaticResponse { status, body },
    };
    net_runtime()
        .http_server_add_route(handle, route)
        .unwrap_or_else(fail_bool)
}

#[no_mangle]
pub extern "C" fn sengoo_http_server_add_ws_echo_route(
    handle: u64,
    path_pattern: *const u8,
) -> i64 {
    reset_last_error();
    let path_pattern = match parse_host(path_pattern) {
        Ok(path) if path.starts_with('/') => path,
        Ok(_) => return fail_bool(NetErrorCode::InvalidArgument),
        Err(code) => return fail_bool(code),
    };
    let route = HttpServerRoute {
        method: "GET".to_string(),
        path_pattern,
        kind: HttpServerRouteKind::WebSocketEcho,
    };
    net_runtime()
        .http_server_add_route(handle, route)
        .unwrap_or_else(fail_bool)
}

#[no_mangle]
pub extern "C" fn sengoo_http_server_add_middleware_require_header(
    handle: u64,
    name: *const u8,
    expected_value: *const u8,
    reject_status: i32,
    reject_body: *const u8,
    reject_body_len: usize,
) -> i64 {
    reset_last_error();
    if !(100..=599).contains(&reject_status) {
        return fail_bool(NetErrorCode::InvalidArgument);
    }
    let name = match parse_host(name) {
        Ok(name) if !name.trim().is_empty() => name.to_ascii_lowercase(),
        Ok(_) => return fail_bool(NetErrorCode::InvalidArgument),
        Err(code) => return fail_bool(code),
    };
    let expected_value = match parse_host(expected_value) {
        Ok(value) => value,
        Err(code) => return fail_bool(code),
    };
    let reject_body = match read_c_buffer(reject_body, reject_body_len) {
        Ok(body) => body,
        Err(code) => return fail_bool(code),
    };

    let middleware = HttpServerMiddleware {
        kind: HttpServerMiddlewareKind::RequireHeader {
            name,
            value: expected_value,
            reject_status,
            reject_body,
        },
    };
    net_runtime()
        .http_server_add_middleware(handle, middleware)
        .unwrap_or_else(fail_bool)
}

#[no_mangle]
pub extern "C" fn sengoo_http_server_serve_once(handle: u64, timeout_ms: u32) -> i64 {
    reset_last_error();
    let (listener, routes, middlewares, max_header_bytes, max_body_bytes) =
        match net_runtime().http_server_snapshot(handle) {
            Ok(snapshot) => snapshot,
            Err(code) => return fail_i64(code),
        };

    let Some(mut stream) = (match accept_with_timeout(&listener, timeout_ms) {
        Ok(stream) => stream,
        Err(code) => return fail_i64(code),
    }) else {
        set_last_error(NetErrorCode::Timeout);
        return 0;
    };

    if let Err(err) = stream.set_read_timeout(Some(connect_timeout(timeout_ms))) {
        return fail_i64(classify_io_error(&err));
    }
    if let Err(err) = stream.set_write_timeout(Some(connect_timeout(timeout_ms))) {
        return fail_i64(classify_io_error(&err));
    }

    match process_http_server_connection(
        &mut stream,
        &routes,
        &middlewares,
        max_header_bytes,
        max_body_bytes,
    ) {
        Ok(()) => 1,
        Err(code) => fail_i64(code),
    }
}

#[no_mangle]
pub extern "C" fn sengoo_http_server_close(handle: u64) -> i64 {
    reset_last_error();
    net_runtime()
        .http_server_close(handle)
        .unwrap_or_else(fail_bool)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{TcpListener, UdpSocket};
    use std::thread;

    fn c_string_bytes(value: &str) -> Vec<u8> {
        let mut bytes = value.as_bytes().to_vec();
        bytes.push(0);
        bytes
    }

    fn send_raw_http_request(port: u16, request: &[u8]) -> Vec<u8> {
        let mut stream =
            TcpStream::connect(("127.0.0.1", port)).expect("connect to test http server");
        stream.write_all(request).expect("write request");
        stream.flush().expect("flush request");
        let mut out = Vec::new();
        stream.read_to_end(&mut out).expect("read response");
        out
    }

    fn parse_http_status_and_body(response: &[u8]) -> (i64, Vec<u8>) {
        let (header, body) = split_http_headers_and_body(response).expect("split response");
        let (status, _) = parse_http_headers(header).expect("parse response header");
        (status, body.to_vec())
    }

    #[test]
    fn tcp_runtime_roundtrip_smoke() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind tcp listener");
        let addr = listener.local_addr().expect("listener addr");
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            let mut buf = [0u8; 64];
            let n = stream.read(&mut buf).expect("server read");
            stream.write_all(&buf[..n]).expect("server write");
        });

        let host = b"127.0.0.1\0";
        let handle = sengoo_tcp_connect(host.as_ptr(), addr.port(), 2_000);
        assert!(handle != 0, "tcp connect should create handle");

        let msg = b"ping";
        let sent = unsafe { sengoo_tcp_send(handle, msg.as_ptr(), msg.len()) };
        assert_eq!(sent, msg.len() as i64, "tcp send should send payload");

        let mut out = [0u8; 16];
        let received = unsafe { sengoo_tcp_recv(handle, out.as_mut_ptr(), out.len(), 2_000) };
        assert_eq!(
            received,
            msg.len() as i64,
            "tcp recv should receive payload"
        );
        assert_eq!(&out[..received as usize], msg);
        assert_eq!(sengoo_tcp_close(handle), 1);

        server.join().expect("server join");
    }

    #[test]
    fn tcp_instance_runtime_roundtrip_smoke() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind tcp listener");
        let addr = listener.local_addr().expect("listener addr");
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            let mut buf = [0u8; 64];
            let n = stream.read(&mut buf).expect("server read");
            stream.write_all(&buf[..n]).expect("server write");
        });

        let rt = NetRuntime::new();
        let handle = rt
            .tcp_connect("127.0.0.1", addr.port(), 2_000)
            .expect("tcp connect should create handle");

        let msg = b"ping";
        let sent = rt.tcp_send(handle, msg).expect("tcp send should succeed");
        assert_eq!(sent, msg.len() as i64);

        let mut out = [0u8; 16];
        match rt
            .tcp_recv(handle, &mut out, 2_000)
            .expect("tcp recv should succeed")
        {
            RecvOutcome::Bytes(received) => {
                assert_eq!(received, msg.len() as i64);
                assert_eq!(&out[..received as usize], msg);
            }
            RecvOutcome::Timeout => panic!("tcp recv should not time out"),
        }
        assert_eq!(rt.tcp_close(handle).expect("tcp close should succeed"), 1);

        server.join().expect("server join");
    }

    #[test]
    fn tcp_instance_runtimes_do_not_share_handles() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind tcp listener");
        let addr = listener.local_addr().expect("listener addr");
        let server = std::thread::spawn(move || {
            let (_stream, _) = listener.accept().expect("accept");
        });

        let rt1 = NetRuntime::new();
        let rt2 = NetRuntime::new();
        let handle = rt1
            .tcp_connect("127.0.0.1", addr.port(), 2_000)
            .expect("tcp connect should create handle");

        assert!(matches!(
            rt2.tcp_close(handle),
            Err(NetErrorCode::HandleNotFound)
        ));
        assert_eq!(rt1.tcp_close(handle).expect("rt1 owns handle"), 1);

        server.join().expect("server join");
    }

    #[test]
    fn udp_instance_runtime_roundtrip_smoke() {
        let server = UdpSocket::bind("127.0.0.1:0").expect("bind udp server");
        server
            .set_read_timeout(Some(Duration::from_millis(2_000)))
            .expect("server timeout");
        let server_addr = server.local_addr().expect("server addr");
        let worker = thread::spawn(move || {
            let mut buf = [0u8; 64];
            let (n, peer) = server.recv_from(&mut buf).expect("server recv");
            server.send_to(&buf[..n], peer).expect("server send");
        });

        let rt = NetRuntime::new();
        let handle = rt
            .udp_bind("127.0.0.1", 0)
            .expect("udp bind should create handle");
        let addr = format!("127.0.0.1:{}", server_addr.port());
        assert_eq!(
            rt.udp_connect(handle, &addr)
                .expect("udp connect should succeed"),
            1
        );

        let msg = b"pong";
        let sent = rt
            .udp_send(handle, msg)
            .expect("udp send should send payload");
        assert_eq!(sent, msg.len() as i64);

        let mut out = [0u8; 16];
        match rt
            .udp_recv(handle, &mut out, 2_000)
            .expect("udp recv should succeed")
        {
            RecvOutcome::Bytes(received) => {
                assert_eq!(received, msg.len() as i64);
                assert_eq!(&out[..received as usize], msg);
            }
            RecvOutcome::Timeout => panic!("udp recv should not time out"),
        }
        assert_eq!(rt.udp_close(handle).expect("udp close should succeed"), 1);

        worker.join().expect("udp worker join");
    }

    #[test]
    fn http_response_instance_lifecycle() {
        let rt = NetRuntime::new();
        let handle = rt
            .http_store(HttpResponseEntry {
                status_code: 202,
                body: b"accepted".to_vec(),
            })
            .expect("http response should store");

        assert_eq!(rt.http_status(handle).expect("status"), 202);
        assert_eq!(rt.http_body_len(handle).expect("body len"), 8);

        let mut out = [0u8; 16];
        let copied = rt
            .http_body_copy(handle, out.as_mut_ptr(), out.len())
            .expect("body copy");
        assert_eq!(copied, 8);
        assert_eq!(&out[..copied as usize], b"accepted");

        let rt2 = NetRuntime::new();
        assert!(matches!(
            rt2.http_status(handle),
            Err(NetErrorCode::HandleNotFound)
        ));
        assert_eq!(rt.http_close(handle).expect("http close"), 1);
        assert!(matches!(
            rt.http_body_len(handle),
            Err(NetErrorCode::HandleNotFound)
        ));
    }

    #[test]
    fn websocket_instance_runtime_echo_roundtrip_smoke() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind ws listener");
        let addr = listener.local_addr().expect("listener addr");
        let worker = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept ws");
            let (opcode, payload) = ws_read_frame(&mut stream).expect("read ws frame");
            assert_eq!(opcode, 0x1);
            assert_eq!(payload, b"ping");
            ws_write_frame(&mut stream, 0x1, &payload, false).expect("echo frame");
            let (close_opcode, _) = ws_read_frame(&mut stream).expect("read close frame");
            assert_eq!(close_opcode, 0x8);
        });

        let client = TcpStream::connect(addr).expect("connect ws peer");
        client
            .set_read_timeout(Some(Duration::from_millis(2_000)))
            .expect("client read timeout");
        client
            .set_write_timeout(Some(Duration::from_millis(2_000)))
            .expect("client write timeout");

        let rt = NetRuntime::new();
        let handle = rt.ws_store(client).expect("ws stream should store");
        let msg = b"ping";
        assert_eq!(
            rt.ws_send_text(handle, msg)
                .expect("ws send should succeed"),
            msg.len() as i64
        );

        let mut out = [0u8; 16];
        let received = rt
            .ws_recv_text(handle, out.as_mut_ptr(), out.len(), 2_000)
            .expect("ws recv should succeed");
        assert_eq!(received, msg.len() as i64);
        assert_eq!(&out[..received as usize], msg);
        assert_eq!(rt.ws_close(handle).expect("ws close should succeed"), 1);

        worker.join().expect("ws worker join");
    }

    #[test]
    fn http_server_instance_state_lifecycle() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind http server");
        listener.set_nonblocking(true).expect("set nonblocking");
        let rt = NetRuntime::new();
        let handle = rt
            .http_server_store(HttpServerState {
                listener,
                routes: Vec::new(),
                middlewares: Vec::new(),
                max_header_bytes: 16 * 1024,
                max_body_bytes: 1024 * 1024,
            })
            .expect("server should store");

        assert!(rt.http_server_local_port(handle).expect("local port") > 0);
        assert_eq!(
            rt.http_server_set_limits(handle, 128, 256)
                .expect("set limits"),
            1
        );
        assert_eq!(
            rt.http_server_add_route(
                handle,
                HttpServerRoute {
                    method: "GET".to_string(),
                    path_pattern: "/hello/:name".to_string(),
                    kind: HttpServerRouteKind::StaticResponse {
                        status: 200,
                        body: b"hello {name}".to_vec(),
                    },
                },
            )
            .expect("add route"),
            1
        );
        assert_eq!(
            rt.http_server_add_middleware(
                handle,
                HttpServerMiddleware {
                    kind: HttpServerMiddlewareKind::RequireHeader {
                        name: "x-auth".to_string(),
                        value: "ok".to_string(),
                        reject_status: 401,
                        reject_body: b"unauthorized".to_vec(),
                    },
                },
            )
            .expect("add middleware"),
            1
        );

        let (_listener, routes, middlewares, max_header_bytes, max_body_bytes) = rt
            .http_server_snapshot(handle)
            .expect("snapshot should exist");
        assert_eq!(max_header_bytes, 128);
        assert_eq!(max_body_bytes, 256);
        assert_eq!(routes.len(), 1);
        assert_eq!(routes[0].method, "GET");
        assert_eq!(routes[0].path_pattern, "/hello/:name");
        match &routes[0].kind {
            HttpServerRouteKind::StaticResponse { status, body } => {
                assert_eq!(*status, 200);
                assert_eq!(body, b"hello {name}");
            }
            HttpServerRouteKind::WebSocketEcho => panic!("expected static response route"),
        }
        assert_eq!(middlewares.len(), 1);
        match &middlewares[0].kind {
            HttpServerMiddlewareKind::RequireHeader {
                name,
                value,
                reject_status,
                reject_body,
            } => {
                assert_eq!(name, "x-auth");
                assert_eq!(value, "ok");
                assert_eq!(*reject_status, 401);
                assert_eq!(reject_body, b"unauthorized");
            }
        }

        let rt2 = NetRuntime::new();
        assert!(matches!(
            rt2.http_server_close(handle),
            Err(NetErrorCode::HandleNotFound)
        ));
        assert_eq!(
            rt.http_server_close(handle)
                .expect("server close should succeed"),
            1
        );
        assert!(matches!(
            rt.http_server_local_port(handle),
            Err(NetErrorCode::HandleNotFound)
        ));
    }

    #[test]
    fn udp_runtime_roundtrip_smoke() {
        let server = UdpSocket::bind("127.0.0.1:0").expect("bind udp server");
        let server_addr = server.local_addr().expect("server addr");
        let worker = std::thread::spawn(move || {
            let mut buf = [0u8; 64];
            let (n, peer) = server.recv_from(&mut buf).expect("server recv");
            server.send_to(&buf[..n], peer).expect("server send");
        });

        let host = b"127.0.0.1\0";
        let handle = sengoo_udp_bind(host.as_ptr(), 0);
        assert!(handle != 0, "udp bind should create handle");
        assert_eq!(
            sengoo_udp_connect(handle, host.as_ptr(), server_addr.port()),
            1,
            "udp connect should succeed"
        );

        let msg = b"pong";
        let sent = unsafe { sengoo_udp_send(handle, msg.as_ptr(), msg.len()) };
        assert_eq!(sent, msg.len() as i64, "udp send should send payload");

        let mut out = [0u8; 16];
        let received = unsafe { sengoo_udp_recv(handle, out.as_mut_ptr(), out.len(), 2_000) };
        assert_eq!(
            received,
            msg.len() as i64,
            "udp recv should receive payload"
        );
        assert_eq!(&out[..received as usize], msg);
        assert_eq!(sengoo_udp_close(handle), 1);

        worker.join().expect("udp worker join");
    }

    #[test]
    fn http_get_runtime_roundtrip_smoke() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind http listener");
        let addr = listener.local_addr().expect("listener addr");
        let worker = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            let mut req = [0u8; 1024];
            let n = stream.read(&mut req).expect("read request");
            let text = String::from_utf8_lossy(&req[..n]);
            assert!(text.starts_with("GET /health HTTP/1.1"));
            let response =
                b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\nConnection: close\r\n\r\nhello";
            stream.write_all(response).expect("write response");
        });

        let url = format!("http://127.0.0.1:{}/health", addr.port());
        let url_c = c_string_bytes(&url);
        let handle = sengoo_http_get(url_c.as_ptr(), 2_000);
        assert!(handle != 0, "http request should produce response handle");
        assert_eq!(sengoo_http_status(handle), 200);
        assert_eq!(sengoo_http_body_len(handle), 5);

        let mut out = [0u8; 16];
        let copied = sengoo_http_body_copy(handle, out.as_mut_ptr(), out.len());
        assert_eq!(copied, 5);
        assert_eq!(&out[..copied as usize], b"hello");
        assert_eq!(sengoo_http_close(handle), 1);

        worker.join().expect("http worker join");
    }

    #[test]
    fn http_chunked_runtime_roundtrip_smoke() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind http listener");
        let addr = listener.local_addr().expect("listener addr");
        let worker = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            let mut req = [0u8; 1024];
            let _ = stream.read(&mut req).expect("read request");
            let response = b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n5\r\nhello\r\n6\r\n world\r\n0\r\n\r\n";
            stream.write_all(response).expect("write response");
        });

        let url = format!("http://127.0.0.1:{}/chunk", addr.port());
        let url_c = c_string_bytes(&url);
        let handle = sengoo_http_get(url_c.as_ptr(), 2_000);
        assert!(handle != 0, "chunked response should produce handle");
        assert_eq!(sengoo_http_status(handle), 200);
        assert_eq!(sengoo_http_body_len(handle), 11);
        let mut out = [0u8; 32];
        let copied = sengoo_http_body_copy(handle, out.as_mut_ptr(), out.len());
        assert_eq!(copied, 11);
        assert_eq!(&out[..11], b"hello world");
        assert_eq!(sengoo_http_close(handle), 1);
        worker.join().expect("join worker");
    }

    #[test]
    fn http_chunked_decode_error_exposes_protocol_error_code() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind http listener");
        let addr = listener.local_addr().expect("listener addr");
        let worker = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            let mut req = [0u8; 1024];
            let _ = stream.read(&mut req).expect("read request");
            let response = b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\nz\r\nbad\r\n0\r\n\r\n";
            stream.write_all(response).expect("write response");
        });

        let url = format!("http://127.0.0.1:{}/bad", addr.port());
        let url_c = c_string_bytes(&url);
        let handle = sengoo_http_get(url_c.as_ptr(), 2_000);
        assert_eq!(handle, 0, "invalid chunk should fail request");
        assert_eq!(sengoo_net_last_error(), SENGOO_NET_ERR_HTTP_CHUNKED);
        worker.join().expect("join worker");
    }

    #[test]
    fn websocket_runtime_echo_roundtrip_smoke() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind ws listener");
        let addr = listener.local_addr().expect("listener addr");
        let worker = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept ws");
            let req = read_http_response_headers(&mut stream).expect("read ws headers");
            let header = std::str::from_utf8(&req).expect("utf8");
            assert!(header.to_ascii_lowercase().contains("upgrade: websocket"));
            assert!(header.starts_with("GET /socket HTTP/1.1"));
            let response = b"HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: test-accept\r\n\r\n";
            stream.write_all(response).expect("write handshake");
            stream.flush().expect("flush handshake");

            let (opcode, payload) = ws_read_frame(&mut stream).expect("read ws frame");
            assert_eq!(opcode, 0x1);
            ws_write_frame(&mut stream, 0x1, &payload, false).expect("echo frame");

            let _ = ws_read_frame(&mut stream); // optional close frame
        });

        let url = format!("ws://127.0.0.1:{}/socket", addr.port());
        let url_c = c_string_bytes(&url);
        let handle = sengoo_ws_connect(url_c.as_ptr(), 2_000);
        assert!(handle != 0, "ws connect should produce handle");

        let msg = b"ping";
        assert_eq!(
            unsafe { sengoo_ws_send_text(handle, msg.as_ptr(), msg.len()) },
            4
        );

        let mut out = [0u8; 16];
        let received = sengoo_ws_recv_text(handle, out.as_mut_ptr(), out.len(), 2_000);
        assert_eq!(received, 4);
        assert_eq!(&out[..received as usize], msg);

        assert_eq!(sengoo_ws_close(handle), 1);
        worker.join().expect("ws worker join");
    }

    #[test]
    fn websocket_handshake_requires_accept_header() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind ws listener");
        let addr = listener.local_addr().expect("listener addr");
        let worker = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept ws");
            let _ = read_http_response_headers(&mut stream).expect("read ws headers");
            let response = b"HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n\r\n";
            stream.write_all(response).expect("write handshake");
            stream.flush().expect("flush handshake");
        });

        let url = format!("ws://127.0.0.1:{}/socket", addr.port());
        let url_c = c_string_bytes(&url);
        let handle = sengoo_ws_connect(url_c.as_ptr(), 2_000);
        assert_eq!(handle, 0, "missing accept header should reject handshake");
        assert_eq!(sengoo_net_last_error(), SENGOO_NET_ERR_WS_HANDSHAKE);
        worker.join().expect("join worker");
    }

    #[test]
    fn websocket_ping_pong_and_close_path() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind ws listener");
        let addr = listener.local_addr().expect("listener addr");
        let worker = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept ws");
            let _ = read_http_response_headers(&mut stream).expect("read ws headers");
            let response = b"HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: demo\r\n\r\n";
            stream.write_all(response).expect("write handshake");
            stream.flush().expect("flush handshake");

            ws_write_frame(&mut stream, 0x9, b"hb", false).expect("send ping");
            ws_write_frame(&mut stream, 0x1, b"ok", false).expect("send text");

            let (opcode, payload) = ws_read_frame(&mut stream).expect("read pong");
            assert_eq!(opcode, 0xA);
            assert_eq!(payload, b"hb");

            let (close_opcode, _) = ws_read_frame(&mut stream).expect("read close");
            assert_eq!(close_opcode, 0x8);
        });

        let url = format!("ws://127.0.0.1:{}/socket", addr.port());
        let url_c = c_string_bytes(&url);
        let handle = sengoo_ws_connect(url_c.as_ptr(), 2_000);
        assert!(handle != 0);
        let mut out = [0u8; 16];
        let n = sengoo_ws_recv_text(handle, out.as_mut_ptr(), out.len(), 2_000);
        assert_eq!(n, 2);
        assert_eq!(&out[..2], b"ok");
        assert_eq!(sengoo_ws_close(handle), 1);
        worker.join().expect("join worker");
    }

    #[test]
    fn http_server_route_and_middleware_pipeline() {
        let host = b"127.0.0.1\0";
        let server = sengoo_http_server_bind(host.as_ptr(), 0);
        assert!(server != 0);
        let port = sengoo_http_server_local_port(server) as u16;
        assert!(port > 0);

        let header_name = b"x-auth\0";
        let header_value = b"ok\0";
        assert_eq!(
            sengoo_http_server_add_middleware_require_header(
                server,
                header_name.as_ptr(),
                header_value.as_ptr(),
                401,
                b"unauthorized".as_ptr(),
                b"unauthorized".len(),
            ),
            1
        );

        let method = b"GET\0";
        let route = b"/hello/:name\0";
        let body = b"hello {name}";
        assert_eq!(
            sengoo_http_server_add_route(
                server,
                method.as_ptr(),
                route.as_ptr(),
                200,
                body.as_ptr(),
                body.len()
            ),
            1
        );

        let first = thread::spawn(move || sengoo_http_server_serve_once(server, 2_000));
        let unauthorized_req =
            b"GET /hello/alice HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n";
        let unauthorized_resp = send_raw_http_request(port, unauthorized_req);
        assert_eq!(first.join().expect("serve once"), 1);
        let (status, body) = parse_http_status_and_body(&unauthorized_resp);
        assert_eq!(status, 401);
        assert_eq!(body, b"unauthorized");

        let second = thread::spawn(move || sengoo_http_server_serve_once(server, 2_000));
        let authorized_req = b"GET /hello/bob HTTP/1.1\r\nHost: localhost\r\nx-auth: ok\r\nConnection: close\r\n\r\n";
        let authorized_resp = send_raw_http_request(port, authorized_req);
        assert_eq!(second.join().expect("serve once"), 1);
        let (status2, body2) = parse_http_status_and_body(&authorized_resp);
        assert_eq!(status2, 200);
        assert_eq!(body2, b"hello bob");

        assert_eq!(sengoo_http_server_close(server), 1);
    }

    #[test]
    fn http_server_rejects_malformed_request_with_bad_request() {
        let host = b"127.0.0.1\0";
        let server = sengoo_http_server_bind(host.as_ptr(), 0);
        assert!(server != 0);
        let port = sengoo_http_server_local_port(server) as u16;
        assert!(port > 0);

        let worker = thread::spawn(move || sengoo_http_server_serve_once(server, 2_000));
        let malformed = b"BROKEN\r\n\r\n";
        let response = send_raw_http_request(port, malformed);
        assert_eq!(worker.join().expect("serve once"), 1);
        let (status, body) = parse_http_status_and_body(&response);
        assert_eq!(status, 400);
        assert_eq!(body, b"bad request");

        assert_eq!(sengoo_http_server_close(server), 1);
    }

    #[test]
    fn http_server_websocket_upgrade_echo_path() {
        let host = b"127.0.0.1\0";
        let server = sengoo_http_server_bind(host.as_ptr(), 0);
        assert!(server != 0);
        let port = sengoo_http_server_local_port(server) as u16;
        assert!(port > 0);

        let ws_path = b"/ws\0";
        assert_eq!(
            sengoo_http_server_add_ws_echo_route(server, ws_path.as_ptr()),
            1
        );

        let worker = thread::spawn(move || sengoo_http_server_serve_once(server, 4_000));
        let mut stream =
            TcpStream::connect(("127.0.0.1", port)).expect("connect ws upgrade test server");
        let handshake = b"GET /ws HTTP/1.1\r\nHost: localhost\r\nConnection: Upgrade\r\nUpgrade: websocket\r\nSec-WebSocket-Version: 13\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\r\n";
        stream.write_all(handshake).expect("write handshake");
        stream.flush().expect("flush handshake");

        let response = read_http_response_headers(&mut stream).expect("read handshake response");
        let (header, _) = split_http_headers_and_body(&response).expect("split response");
        let (status, headers) = parse_http_headers(header).expect("parse headers");
        assert_eq!(status, 101);
        assert!(headers.contains_key("sec-websocket-accept"));

        ws_write_frame(&mut stream, 0x9, b"hb", true).expect("write ping");
        let (pong_opcode, pong_payload) = ws_read_frame(&mut stream).expect("read pong");
        assert_eq!(pong_opcode, 0xA);
        assert_eq!(pong_payload, b"hb");

        ws_write_frame(&mut stream, 0x1, b"echo", true).expect("write text");
        let (text_opcode, text_payload) = ws_read_frame(&mut stream).expect("read text");
        assert_eq!(text_opcode, 0x1);
        assert_eq!(text_payload, b"echo");

        ws_write_frame(&mut stream, 0x8, &[], true).expect("write close");
        let (close_opcode, _) = ws_read_frame(&mut stream).expect("read close");
        assert_eq!(close_opcode, 0x8);
        assert_eq!(worker.join().expect("serve once"), 1);

        assert_eq!(sengoo_http_server_close(server), 1);
    }

    #[test]
    fn invalid_argument_sets_error_mapping() {
        let ret = unsafe { sengoo_tcp_send(999, std::ptr::null(), 5) };
        assert_eq!(ret, -1);
        assert_eq!(sengoo_net_last_error(), SENGOO_NET_ERR_INVALID_ARGUMENT);

        let mut name_buf = [0u8; 32];
        let copied = sengoo_net_error_name_copy(
            SENGOO_NET_ERR_INVALID_ARGUMENT,
            name_buf.as_mut_ptr(),
            name_buf.len(),
        );
        assert!(copied > 0);
        assert_eq!(&name_buf[..copied as usize], b"invalid_argument");
    }

    #[test]
    fn net_last_error_remains_process_visible_across_threads() {
        use std::sync::atomic::AtomicBool;

        sengoo_net_clear_error();
        assert_eq!(sengoo_net_last_error(), SENGOO_NET_ERR_OK);

        let stop = std::sync::Arc::new(AtomicBool::new(false));
        let worker_stop = stop.clone();
        let (attempt_tx, attempt_rx) = std::sync::mpsc::channel();
        let worker = thread::spawn(move || {
            while !worker_stop.load(Ordering::Relaxed) {
                let ret = unsafe { sengoo_tcp_send(999, std::ptr::null(), 5) };
                assert_eq!(ret, -1);
                let _ = attempt_tx.send(());
                thread::yield_now();
            }
        });

        let mut observed_worker_error = false;
        for _ in 0..1024 {
            attempt_rx
                .recv_timeout(Duration::from_millis(50))
                .expect("worker should set a process-visible net error");
            if sengoo_net_last_error() == SENGOO_NET_ERR_INVALID_ARGUMENT {
                observed_worker_error = true;
                stop.store(true, Ordering::Relaxed);
                break;
            }
        }
        stop.store(true, Ordering::Relaxed);
        worker.join().expect("worker should not panic");
        assert!(observed_worker_error);
    }
}
