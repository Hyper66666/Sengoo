use std::collections::HashMap;
use std::io::ErrorKind;
use std::net::{TcpStream, ToSocketAddrs, UdpSocket};
use std::sync::atomic::{AtomicI32, AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

#[cfg(test)]
static NET_TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

#[cfg(test)]
pub(crate) fn net_test_lock() -> std::sync::MutexGuard<'static, ()> {
    NET_TEST_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

mod http_client;
mod http_server;
mod tcp;
mod tls;
mod udp;
mod websocket;
pub use http_client::{
    sengoo_http_body_copy, sengoo_http_body_len, sengoo_http_close, sengoo_http_get,
    sengoo_http_post, sengoo_http_status,
};
pub use http_server::{
    sengoo_http_request_begin_stream, sengoo_http_request_begin_stream_with_length,
    sengoo_http_request_body_copy, sengoo_http_request_body_len, sengoo_http_request_close,
    sengoo_http_request_header_copy, sengoo_http_request_header_len,
    sengoo_http_request_method_copy, sengoo_http_request_method_len, sengoo_http_request_path_copy,
    sengoo_http_request_path_len, sengoo_http_request_query_copy, sengoo_http_request_query_len,
    sengoo_http_request_respond, sengoo_http_request_respond_with_content_type,
    sengoo_http_request_version_copy, sengoo_http_request_version_len,
    sengoo_http_response_stream_close, sengoo_http_response_stream_finish,
    sengoo_http_response_stream_write, sengoo_http_server_add_middleware_require_header,
    sengoo_http_server_add_route, sengoo_http_server_add_ws_echo_route, sengoo_http_server_bind,
    sengoo_http_server_bind_tls, sengoo_http_server_claim_serve_mode, sengoo_http_server_close,
    sengoo_http_server_local_port, sengoo_http_server_next_request,
    sengoo_http_server_next_request_async__cancel, sengoo_http_server_next_request_async__drop,
    sengoo_http_server_next_request_async__poll, sengoo_http_server_next_request_async__result,
    sengoo_http_server_next_request_async__start, sengoo_http_server_serve_once,
    sengoo_http_server_set_keep_alive, sengoo_http_server_set_limits, HttpServerNextRequestResult,
};
use http_server::{HttpRequestEntry, HttpServerState};
#[cfg(test)]
use http_server::{
    HttpServerMiddleware, HttpServerMiddlewareKind, HttpServerRoute, HttpServerRouteKind,
};
pub use tcp::{sengoo_tcp_close, sengoo_tcp_connect, sengoo_tcp_recv, sengoo_tcp_send};
pub(crate) use tls::TlsStream;
pub use udp::{
    sengoo_udp_bind, sengoo_udp_close, sengoo_udp_connect, sengoo_udp_recv, sengoo_udp_send,
};
#[cfg(test)]
use websocket::{read_http_response_headers, ws_read_frame, ws_write_frame};
pub use websocket::{sengoo_ws_close, sengoo_ws_connect, sengoo_ws_recv_text, sengoo_ws_send_text};

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
    TlsCertInvalid = 15,
    TlsHostnameMismatch = 16,
    TlsHandshake = 17,
    TlsUnavailable = 18,
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
pub const SENGOO_NET_ERR_TLS_CERT_INVALID: i32 = NetErrorCode::TlsCertInvalid as i32;
pub const SENGOO_NET_ERR_TLS_HOSTNAME_MISMATCH: i32 = NetErrorCode::TlsHostnameMismatch as i32;
pub const SENGOO_NET_ERR_TLS_HANDSHAKE: i32 = NetErrorCode::TlsHandshake as i32;
pub const SENGOO_NET_ERR_TLS_UNAVAILABLE: i32 = NetErrorCode::TlsUnavailable as i32;

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

fn status_from_net_error_code(code: NetErrorCode) -> i64 {
    match code {
        NetErrorCode::Ok => 0,
        NetErrorCode::InvalidArgument | NetErrorCode::InvalidUrl => 2,
        NetErrorCode::HandleNotFound => 3,
        NetErrorCode::UnsupportedScheme => 8,
        NetErrorCode::ResolveFailed
        | NetErrorCode::ConnectFailed
        | NetErrorCode::IoError
        | NetErrorCode::WebSocketHandshakeError
        | NetErrorCode::RemoteClosed => 9,
        NetErrorCode::HttpProtocolError
        | NetErrorCode::HttpChunkDecodeError
        | NetErrorCode::WebSocketProtocolError => 10,
        NetErrorCode::Timeout => 11,
        NetErrorCode::InternalError => 1,
        NetErrorCode::TlsCertInvalid => 15,
        NetErrorCode::TlsHostnameMismatch => 16,
        NetErrorCode::TlsHandshake => 17,
        NetErrorCode::TlsUnavailable => 18,
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

#[derive(Debug)]
struct NetRuntime {
    next_handle: AtomicU64,
    tcp_streams: Mutex<HashMap<u64, TcpStream>>,
    udp_sockets: Mutex<HashMap<u64, UdpSocket>>,
    http_responses: Mutex<HashMap<u64, HttpResponseEntry>>,
    ws_streams: Mutex<HashMap<u64, TcpStream>>,
    http_servers: Mutex<HashMap<u64, HttpServerState>>,
    http_requests: Mutex<HashMap<u64, HttpRequestEntry>>,
    http_response_streams: Mutex<HashMap<u64, http_server::HttpResponseStreamEntry>>,
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
            http_requests: Mutex::new(HashMap::new()),
            http_response_streams: Mutex::new(HashMap::new()),
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
        if let Ok(mut table) = self.http_requests.lock() {
            table.clear();
        }
        if let Ok(mut table) = self.http_response_streams.lock() {
            table.clear();
        }
        reset_last_error();
    }

    fn alloc_handle(&self) -> u64 {
        self.next_handle.fetch_add(1, Ordering::Relaxed)
    }
}

fn net_runtime() -> &'static NetRuntime {
    NET_RUNTIME.get_or_init(NetRuntime::new)
}

/// Returns the error code widened to `i64` to match the stdlib extern ABI
/// (`fn sengoo_net_last_error() -> i64`) on every architecture.
#[no_mangle]
pub extern "C" fn sengoo_net_last_error() -> i64 {
    i64::from(LAST_NET_ERROR.load(Ordering::Relaxed))
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
        SENGOO_NET_ERR_TLS_CERT_INVALID => "tls_cert_invalid",
        SENGOO_NET_ERR_TLS_HOSTNAME_MISMATCH => "tls_hostname_mismatch",
        SENGOO_NET_ERR_TLS_HANDSHAKE => "tls_handshake",
        SENGOO_NET_ERR_TLS_UNAVAILABLE => "tls_unavailable",
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
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
        let _guard = net_test_lock();
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
        assert_eq!(sengoo_tcp_close(handle), 1);

        server.join().expect("server join");
    }

    #[test]
    fn tcp_instance_runtime_roundtrip_smoke() {
        let _guard = net_test_lock();
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
        let _guard = net_test_lock();
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
        let _guard = net_test_lock();
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
        let _guard = net_test_lock();
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
        let _guard = net_test_lock();
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
        let _guard = net_test_lock();
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
                serve_mode: 0,
                keep_alive_enabled: false,
                live_connection: None,
                tls: None,
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

        let (_listener, routes, middlewares, max_header_bytes, max_body_bytes, keep_alive) = rt
            .http_server_snapshot(handle)
            .expect("snapshot should exist");
        assert_eq!(max_header_bytes, 128);
        assert_eq!(max_body_bytes, 256);
        assert!(!keep_alive);
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
        let _guard = net_test_lock();
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
        assert_eq!(sengoo_udp_close(handle), 1);

        worker.join().expect("udp worker join");
    }

    #[test]
    fn http_get_runtime_roundtrip_smoke() {
        let _guard = net_test_lock();
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
        assert_eq!(sengoo_http_close(handle), 1);

        worker.join().expect("http worker join");
    }

    #[test]
    fn http_chunked_runtime_roundtrip_smoke() {
        let _guard = net_test_lock();
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
        let _guard = net_test_lock();
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
        assert_eq!(
            sengoo_net_last_error(),
            i64::from(SENGOO_NET_ERR_HTTP_CHUNKED)
        );
        worker.join().expect("join worker");
    }

    #[test]
    fn http_ftp_scheme_returns_unsupported() {
        let _guard = net_test_lock();
        let url = c_string_bytes("ftp://127.0.0.1/");
        let handle = sengoo_http_get(url.as_ptr(), 1_000);
        assert_eq!(handle, 0, "ftp scheme should remain unsupported");
        assert_eq!(
            sengoo_net_last_error(),
            i64::from(SENGOO_NET_ERR_UNSUPPORTED_SCHEME)
        );
    }

    #[test]
    fn websocket_runtime_echo_roundtrip_smoke() {
        let _guard = net_test_lock();
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
        assert_eq!(sengoo_ws_close(handle), 1);
        worker.join().expect("ws worker join");
    }

    #[test]
    fn websocket_handshake_requires_accept_header() {
        let _guard = net_test_lock();
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
        assert_eq!(
            sengoo_net_last_error(),
            i64::from(SENGOO_NET_ERR_WS_HANDSHAKE)
        );
        worker.join().expect("join worker");
    }

    #[test]
    fn websocket_ping_pong_and_close_path() {
        let _guard = net_test_lock();
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
        let _guard = net_test_lock();
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
        assert_eq!(sengoo_http_server_close(server), 1);
    }

    #[test]
    fn http_server_rejects_malformed_request_with_bad_request() {
        let _guard = net_test_lock();
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
        let _guard = net_test_lock();
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
        let _guard = net_test_lock();
        let ret = unsafe { sengoo_tcp_send(999, std::ptr::null(), 5) };
        assert_eq!(ret, -1);
        assert_eq!(
            sengoo_net_last_error(),
            i64::from(SENGOO_NET_ERR_INVALID_ARGUMENT)
        );

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
        let _guard = net_test_lock();
        use std::sync::atomic::AtomicBool;

        sengoo_net_clear_error();
        assert_eq!(sengoo_net_last_error(), i64::from(SENGOO_NET_ERR_OK));

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
            if sengoo_net_last_error() == i64::from(SENGOO_NET_ERR_INVALID_ARGUMENT) {
                observed_worker_error = true;
                stop.store(true, Ordering::Relaxed);
                break;
            }
        }
        stop.store(true, Ordering::Relaxed);
        worker.join().expect("worker should not panic");
        assert!(observed_worker_error);
    }

    fn read_request_text(
        handle: u64,
        len_fn: extern "C" fn(u64) -> i64,
        copy_fn: extern "C" fn(u64, *mut u8, usize) -> i64,
    ) -> String {
        let len = len_fn(handle);
        assert!(len >= 0, "text length should be readable");
        let mut buf = vec![0u8; len as usize];
        let copied = copy_fn(handle, buf.as_mut_ptr(), buf.len());
        assert_eq!(copied, len, "copy should return full length");
        String::from_utf8(buf).expect("request text should be utf-8")
    }

    #[test]
    fn http_server_next_request_pull_and_respond_roundtrip() {
        let _guard = net_test_lock();
        let host = b"127.0.0.1\0";
        let server = sengoo_http_server_bind(host.as_ptr(), 0);
        assert!(server != 0);
        let port = sengoo_http_server_local_port(server) as u16;

        let client = thread::spawn(move || {
            let request = b"POST /items?limit=5 HTTP/1.1\r\nHost: localhost\r\nX-Trace: abc\r\nContent-Length: 4\r\n\r\nping";
            send_raw_http_request(port, request)
        });

        let request = sengoo_http_server_next_request(server, 4_000);
        assert!(request != 0, "dynamic request should surface a handle");

        assert_eq!(
            read_request_text(
                request,
                sengoo_http_request_method_len,
                sengoo_http_request_method_copy
            ),
            "POST"
        );
        assert_eq!(
            read_request_text(
                request,
                sengoo_http_request_path_len,
                sengoo_http_request_path_copy
            ),
            "/items"
        );
        assert_eq!(
            read_request_text(
                request,
                sengoo_http_request_query_len,
                sengoo_http_request_query_copy
            ),
            "limit=5"
        );
        assert_eq!(
            read_request_text(
                request,
                sengoo_http_request_version_len,
                sengoo_http_request_version_copy
            ),
            "HTTP/1.1"
        );

        let mixed_case_name = c_string_bytes("X-Trace");
        let header_len = sengoo_http_request_header_len(request, mixed_case_name.as_ptr());
        assert_eq!(header_len, 3, "header lookup should be case-insensitive");
        let mut header_buf = [0u8; 8];
        let header_copied = sengoo_http_request_header_copy(
            request,
            mixed_case_name.as_ptr(),
            header_buf.as_mut_ptr(),
            header_buf.len(),
        );
        assert_eq!(header_copied, 3);
        assert_eq!(&header_buf[..3], b"abc");

        let missing_name = c_string_bytes("x-missing");
        assert_eq!(
            sengoo_http_request_header_len(request, missing_name.as_ptr()),
            -1,
            "absent header should be distinguishable from an empty value"
        );
        assert_eq!(sengoo_net_last_error(), i64::from(SENGOO_NET_ERR_OK));

        assert_eq!(sengoo_http_request_body_len(request), 4);
        let mut body_buf = [0u8; 8];
        let body_copied =
            sengoo_http_request_body_copy(request, body_buf.as_mut_ptr(), body_buf.len());
        assert_eq!(body_copied, 4);
        assert_eq!(&body_buf[..4], b"ping");

        let content_type = c_string_bytes("application/json");
        let payload = b"{\"ok\":true}";
        assert_eq!(
            sengoo_http_request_respond_with_content_type(
                request,
                200,
                content_type.as_ptr(),
                payload.as_ptr(),
                payload.len(),
            ),
            1
        );

        let response = client.join().expect("client thread");
        let (header, body) = split_http_headers_and_body(&response).expect("split response");
        let (status, headers) = parse_http_headers(header).expect("parse headers");
        assert_eq!(status, 200);
        assert_eq!(body, payload);
        assert_eq!(
            headers.get("content-type").map(String::as_str),
            Some("application/json")
        );

        assert_eq!(sengoo_http_server_close(server), 1);
    }

    #[test]
    fn http_server_next_request_timeout_maps_to_timeout() {
        let _guard = net_test_lock();
        let host = b"127.0.0.1\0";
        let server = sengoo_http_server_bind(host.as_ptr(), 0);
        assert!(server != 0);
        let port = sengoo_http_server_local_port(server) as u16;

        let request = sengoo_http_server_next_request(server, 60);
        assert_eq!(request, 0, "no request should map to timeout");
        assert_eq!(sengoo_net_last_error(), i64::from(SENGOO_NET_ERR_TIMEOUT));

        let client = thread::spawn(move || {
            let request = b"GET /later HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n";
            send_raw_http_request(port, request)
        });
        let request = sengoo_http_server_next_request(server, 4_000);
        assert!(request != 0, "a later pull on the same server should work");
        assert_eq!(
            sengoo_http_request_respond(request, 200, b"late".as_ptr(), 4),
            1
        );
        let (status, body) = parse_http_status_and_body(&client.join().expect("client"));
        assert_eq!(status, 200);
        assert_eq!(body, b"late");

        assert_eq!(sengoo_http_server_close(server), 1);
    }

    fn poll_http_next_request_until_ready(handle: i64, timeout_ms: u64) -> i64 {
        let deadline = std::time::Instant::now() + Duration::from_millis(timeout_ms);
        loop {
            let poll = unsafe { sengoo_http_server_next_request_async__poll(handle) };
            if poll != 0 {
                return poll;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "async HTTP next-request future should become ready before test deadline"
            );
            thread::sleep(Duration::from_millis(2));
        }
    }

    #[test]
    fn http_server_next_request_async_timeout_preserves_server() {
        let _guard = net_test_lock();
        let host = b"127.0.0.1\0";
        let server = sengoo_http_server_bind(host.as_ptr(), 0);
        assert!(server != 0);
        let port = sengoo_http_server_local_port(server) as u16;

        let future = sengoo_http_server_next_request_async__start(server, 40);
        assert!(future != 0);
        assert_eq!(poll_http_next_request_until_ready(future, 1_000), 1);
        let result = unsafe { sengoo_http_server_next_request_async__result(future) };
        assert!(!result.is_ok);
        assert_eq!(result.value.handle, 0);
        assert_eq!(result.error, 11);

        let client = thread::spawn(move || {
            let request =
                b"GET /after-timeout HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n";
            send_raw_http_request(port, request)
        });
        let request = sengoo_http_server_next_request(server, 4_000);
        assert!(
            request != 0,
            "server should remain usable after async timeout"
        );
        assert_eq!(
            sengoo_http_request_respond(request, 200, b"late".as_ptr(), 4),
            1
        );
        let (status, body) = parse_http_status_and_body(&client.join().expect("client"));
        assert_eq!(status, 200);
        assert_eq!(body, b"late");

        assert_eq!(sengoo_http_server_close(server), 1);
    }

    #[test]
    fn http_server_next_request_async_ready_on_localhost_client() {
        let _guard = net_test_lock();
        let host = b"127.0.0.1\0";
        let server = sengoo_http_server_bind(host.as_ptr(), 0);
        assert!(server != 0);
        let port = sengoo_http_server_local_port(server) as u16;

        let future = sengoo_http_server_next_request_async__start(server, 4_000);
        assert!(future != 0);
        let client = thread::spawn(move || {
            let request =
                b"GET /async?x=1 HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n";
            send_raw_http_request(port, request)
        });

        assert_eq!(poll_http_next_request_until_ready(future, 4_000), 1);
        let result = unsafe { sengoo_http_server_next_request_async__result(future) };
        assert!(
            result.is_ok,
            "async result should be ok, got {}",
            result.error
        );
        assert!(result.value.handle > 0);
        assert_eq!(result.error, 0);
        assert_eq!(
            read_request_text(
                result.value.handle as u64,
                sengoo_http_request_path_len,
                sengoo_http_request_path_copy,
            ),
            "/async"
        );
        assert_eq!(
            sengoo_http_request_respond(result.value.handle as u64, 200, b"ok".as_ptr(), 2),
            1
        );
        let (status, body) = parse_http_status_and_body(&client.join().expect("client"));
        assert_eq!(status, 200);
        assert_eq!(body, b"ok");

        assert_eq!(sengoo_http_server_close(server), 1);
    }

    #[test]
    fn http_server_next_request_async_drop_pending_leaves_server_usable() {
        let _guard = net_test_lock();
        let host = b"127.0.0.1\0";
        let server = sengoo_http_server_bind(host.as_ptr(), 0);
        assert!(server != 0);
        let port = sengoo_http_server_local_port(server) as u16;

        let interest_count_before = crate::async_runtime::http_listener_interest_count();
        let dropped = sengoo_http_server_next_request_async__start(server, 4_000);
        assert!(dropped != 0);
        assert_eq!(
            crate::async_runtime::http_listener_interest_count(),
            interest_count_before + 1,
            "starting the future should register one listener interest"
        );
        assert_eq!(
            unsafe { sengoo_http_server_next_request_async__poll(dropped) },
            0
        );
        unsafe { sengoo_http_server_next_request_async__drop(dropped) };
        assert_eq!(
            crate::async_runtime::http_listener_interest_count(),
            interest_count_before,
            "dropping the future should unregister its listener interest"
        );

        let future = sengoo_http_server_next_request_async__start(server, 4_000);
        assert!(future != 0);
        let client = thread::spawn(move || {
            let request =
                b"GET /after-drop HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n";
            send_raw_http_request(port, request)
        });
        assert_eq!(poll_http_next_request_until_ready(future, 4_000), 1);
        let result = unsafe { sengoo_http_server_next_request_async__result(future) };
        assert!(result.is_ok);
        assert!(result.value.handle > 0);
        assert_eq!(
            sengoo_http_request_respond(result.value.handle as u64, 200, b"ok".as_ptr(), 2),
            1
        );
        let (status, body) = parse_http_status_and_body(&client.join().expect("client"));
        assert_eq!(status, 200);
        assert_eq!(body, b"ok");

        assert_eq!(sengoo_http_server_close(server), 1);
    }

    fn assert_ready_http_future_abandonment_releases_request(cancel: bool) {
        let _guard = net_test_lock();
        let host = b"127.0.0.1\0";
        let server = sengoo_http_server_bind(host.as_ptr(), 0);
        assert!(server != 0);
        let port = sengoo_http_server_local_port(server) as u16;
        let table_len_before = net_runtime().http_request_table_len();

        let future = sengoo_http_server_next_request_async__start(server, 4_000);
        assert!(future != 0);
        let client = thread::spawn(move || {
            let request = b"POST /abandoned-ready HTTP/1.1\r\nHost: localhost\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok";
            send_raw_http_request(port, request)
        });
        assert_eq!(poll_http_next_request_until_ready(future, 4_000), 1);
        assert_eq!(
            net_runtime().http_request_pending_count(server).unwrap(),
            1,
            "the ready future owns one unpublished request"
        );

        if cancel {
            assert!(unsafe { sengoo_http_server_next_request_async__cancel(future) });
        } else {
            unsafe { sengoo_http_server_next_request_async__drop(future) };
        }

        let table_len_after = net_runtime().http_request_table_len();
        let pending_after = net_runtime().http_request_pending_count(server).unwrap();
        if table_len_after != table_len_before || pending_after != 0 {
            assert_eq!(
                sengoo_http_server_close(server),
                1,
                "RED cleanup must drain a leaked unpublished request"
            );
        }
        let (status, body) = parse_http_status_and_body(&client.join().expect("client"));
        assert_eq!(status, 504);
        assert_eq!(body, b"gateway timeout");
        assert_eq!(
            table_len_after, table_len_before,
            "abandoning a ready future must remove its unpublished request handle"
        );
        assert_eq!(pending_after, 0);

        let next_client = thread::spawn(move || {
            let request = b"GET /after-ready-abandon HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n";
            send_raw_http_request(port, request)
        });
        let next = sengoo_http_server_next_request(server, 4_000);
        assert!(
            next != 0,
            "server must remain usable after ready abandonment"
        );
        assert_eq!(sengoo_http_request_respond(next, 200, b"ok".as_ptr(), 2), 1);
        let (next_status, next_body) =
            parse_http_status_and_body(&next_client.join().expect("next client"));
        assert_eq!(next_status, 200);
        assert_eq!(next_body, b"ok");
        assert_eq!(sengoo_http_server_close(server), 1);
    }

    #[test]
    fn http_server_next_request_async_drop_ready_releases_unpublished_request() {
        assert_ready_http_future_abandonment_releases_request(false);
    }

    #[test]
    fn http_server_next_request_async_cancel_ready_releases_unpublished_request() {
        assert_ready_http_future_abandonment_releases_request(true);
    }

    #[test]
    fn http_server_next_request_async_slow_client_never_publishes_partial_request() {
        let _guard = net_test_lock();
        use std::io::Write;

        let host = b"127.0.0.1\0";
        let server = sengoo_http_server_bind(host.as_ptr(), 0);
        assert!(server != 0);
        let port = sengoo_http_server_local_port(server) as u16;
        let future = sengoo_http_server_next_request_async__start(server, 4_000);
        assert!(future != 0);

        let mut slow_client = TcpStream::connect(("127.0.0.1", port)).expect("slow client");
        slow_client
            .write_all(b"POST /partial HTTP/1.1\r\nHost: localhost\r\nContent-Length: 8\r\n")
            .expect("partial request should be writable");
        thread::sleep(Duration::from_millis(10));
        assert_eq!(
            unsafe { sengoo_http_server_next_request_async__poll(future) },
            0,
            "an incomplete request must not surface a request handle"
        );

        let client = thread::spawn(move || {
            let request =
                b"GET /after-slow HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n";
            send_raw_http_request(port, request)
        });
        assert_eq!(poll_http_next_request_until_ready(future, 4_000), 1);
        let result = unsafe { sengoo_http_server_next_request_async__result(future) };
        assert!(result.is_ok);
        assert_eq!(
            read_request_text(
                result.value.handle as u64,
                sengoo_http_request_path_len,
                sengoo_http_request_path_copy,
            ),
            "/after-slow"
        );
        assert_eq!(
            sengoo_http_request_respond(result.value.handle as u64, 200, b"ok".as_ptr(), 2),
            1
        );
        let (status, body) = parse_http_status_and_body(&client.join().expect("client"));
        assert_eq!(status, 200);
        assert_eq!(body, b"ok");

        drop(slow_client);
        assert_eq!(sengoo_http_server_close(server), 1);
    }

    #[test]
    fn http_server_next_request_answers_static_and_middleware_inline() {
        let _guard = net_test_lock();
        let host = b"127.0.0.1\0";
        let server = sengoo_http_server_bind(host.as_ptr(), 0);
        assert!(server != 0);
        let port = sengoo_http_server_local_port(server) as u16;

        let header_name = c_string_bytes("x-auth");
        let header_value = c_string_bytes("ok");
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
        let method = c_string_bytes("GET");
        let route = c_string_bytes("/health");
        assert_eq!(
            sengoo_http_server_add_route(
                server,
                method.as_ptr(),
                route.as_ptr(),
                200,
                b"healthy".as_ptr(),
                b"healthy".len(),
            ),
            1
        );

        let static_client = thread::spawn(move || {
            let request =
                b"GET /health HTTP/1.1\r\nHost: localhost\r\nx-auth: ok\r\nConnection: close\r\n\r\n";
            send_raw_http_request(port, request)
        });
        thread::sleep(Duration::from_millis(100));
        let rejected_client = thread::spawn(move || {
            let request = b"GET /dyn HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n";
            send_raw_http_request(port, request)
        });
        thread::sleep(Duration::from_millis(100));
        let dynamic_client = thread::spawn(move || {
            let request =
                b"GET /dyn HTTP/1.1\r\nHost: localhost\r\nx-auth: ok\r\nConnection: close\r\n\r\n";
            send_raw_http_request(port, request)
        });

        let request = sengoo_http_server_next_request(server, 6_000);
        assert!(
            request != 0,
            "only the unmatched request should surface as dynamic"
        );
        assert_eq!(
            read_request_text(
                request,
                sengoo_http_request_path_len,
                sengoo_http_request_path_copy
            ),
            "/dyn"
        );

        let (static_status, static_body) =
            parse_http_status_and_body(&static_client.join().expect("static client"));
        assert_eq!(static_status, 200);
        assert_eq!(static_body, b"healthy");

        let (rejected_status, rejected_body) =
            parse_http_status_and_body(&rejected_client.join().expect("rejected client"));
        assert_eq!(rejected_status, 401);
        assert_eq!(rejected_body, b"unauthorized");

        assert_eq!(
            sengoo_http_request_respond(request, 200, b"dyn".as_ptr(), 3),
            1
        );
        let (dynamic_status, dynamic_body) =
            parse_http_status_and_body(&dynamic_client.join().expect("dynamic client"));
        assert_eq!(dynamic_status, 200);
        assert_eq!(dynamic_body, b"dyn");

        assert_eq!(sengoo_http_server_close(server), 1);
    }

    #[test]
    fn http_request_double_respond_is_rejected() {
        let _guard = net_test_lock();
        let host = b"127.0.0.1\0";
        let server = sengoo_http_server_bind(host.as_ptr(), 0);
        assert!(server != 0);
        let port = sengoo_http_server_local_port(server) as u16;

        let client = thread::spawn(move || {
            let request = b"GET /once HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n";
            send_raw_http_request(port, request)
        });
        let request = sengoo_http_server_next_request(server, 4_000);
        assert!(request != 0);

        assert_eq!(
            sengoo_http_request_respond(request, 200, b"one".as_ptr(), 3),
            1
        );
        assert_eq!(
            sengoo_http_request_respond(request, 200, b"two".as_ptr(), 3),
            0,
            "double respond must be rejected"
        );
        assert_eq!(
            sengoo_net_last_error(),
            i64::from(SENGOO_NET_ERR_HANDLE_NOT_FOUND)
        );

        let (status, body) = parse_http_status_and_body(&client.join().expect("client"));
        assert_eq!(status, 200);
        assert_eq!(body, b"one", "no second response bytes may be written");

        assert_eq!(sengoo_http_server_close(server), 1);
    }

    #[test]
    fn http_request_close_unanswered_sends_gateway_timeout() {
        let _guard = net_test_lock();
        let host = b"127.0.0.1\0";
        let server = sengoo_http_server_bind(host.as_ptr(), 0);
        assert!(server != 0);
        let port = sengoo_http_server_local_port(server) as u16;

        let client = thread::spawn(move || {
            let request = b"GET /never HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n";
            send_raw_http_request(port, request)
        });
        let request = sengoo_http_server_next_request(server, 4_000);
        assert!(request != 0);

        assert_eq!(sengoo_http_request_close(request), 1);
        let (status, body) = parse_http_status_and_body(&client.join().expect("client"));
        assert_eq!(status, 504);
        assert_eq!(body, b"gateway timeout");

        assert_eq!(
            sengoo_http_request_close(request),
            1,
            "closing an already released request must be idempotent"
        );
        assert_eq!(sengoo_net_last_error(), i64::from(SENGOO_NET_ERR_OK));

        assert_eq!(sengoo_http_server_close(server), 1);
    }

    #[test]
    fn http_request_oversized_response_body_is_rejected() {
        let _guard = net_test_lock();
        let host = b"127.0.0.1\0";
        let server = sengoo_http_server_bind(host.as_ptr(), 0);
        assert!(server != 0);
        assert_eq!(sengoo_http_server_set_limits(server, 16 * 1024, 8), 1);
        let port = sengoo_http_server_local_port(server) as u16;

        let client = thread::spawn(move || {
            let request = b"GET /small HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n";
            send_raw_http_request(port, request)
        });
        let request = sengoo_http_server_next_request(server, 4_000);
        assert!(request != 0);

        let oversized = b"123456789";
        assert_eq!(
            sengoo_http_request_respond(request, 200, oversized.as_ptr(), oversized.len()),
            0,
            "response body above max_body_bytes must be rejected"
        );
        assert_eq!(
            sengoo_net_last_error(),
            i64::from(SENGOO_NET_ERR_INVALID_ARGUMENT)
        );

        assert_eq!(
            sengoo_http_request_respond(request, 200, b"ok".as_ptr(), 2),
            1,
            "handle must stay answerable after the rejection"
        );
        let (status, body) = parse_http_status_and_body(&client.join().expect("client"));
        assert_eq!(status, 200);
        assert_eq!(body, b"ok");

        assert_eq!(sengoo_http_server_close(server), 1);
    }

    #[test]
    fn http_server_pending_cap_answers_overflow_inline() {
        let _guard = net_test_lock();
        let host = b"127.0.0.1\0";
        let server = sengoo_http_server_bind(host.as_ptr(), 0);
        assert!(server != 0);
        let port = sengoo_http_server_local_port(server) as u16;

        let mut clients = Vec::new();
        for index in 0..64 {
            clients.push(thread::spawn(move || {
                let request = format!(
                    "GET /pending/{index} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n"
                );
                send_raw_http_request(port, request.as_bytes())
            }));
        }

        let mut pulled = Vec::new();
        for _ in 0..64 {
            let request = sengoo_http_server_next_request(server, 8_000);
            assert!(request != 0, "all 64 requests should be pullable");
            pulled.push(request);
        }
        assert_eq!(
            net_runtime()
                .http_request_pending_count(server)
                .expect("pending count"),
            64
        );

        let overflow_client = thread::spawn(move || {
            let request = b"GET /overflow HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n";
            send_raw_http_request(port, request)
        });
        let no_handle = sengoo_http_server_next_request(server, 500);
        assert_eq!(no_handle, 0, "overflow request must not surface a handle");
        assert_eq!(sengoo_net_last_error(), i64::from(SENGOO_NET_ERR_TIMEOUT));
        let (overflow_status, overflow_body) =
            parse_http_status_and_body(&overflow_client.join().expect("overflow client"));
        assert_eq!(overflow_status, 503);
        assert_eq!(overflow_body, b"service unavailable");

        for (index, request) in pulled.iter().enumerate() {
            let body = format!("answer {index}");
            assert_eq!(
                sengoo_http_request_respond(*request, 200, body.as_ptr(), body.len()),
                1,
                "previously pulled handles must stay answerable"
            );
        }
        for client in clients {
            let (status, _) = parse_http_status_and_body(&client.join().expect("pending client"));
            assert_eq!(status, 200);
        }

        assert_eq!(sengoo_http_server_close(server), 1);
    }

    #[test]
    fn http_server_close_drains_unanswered_requests_with_gateway_timeout() {
        let _guard = net_test_lock();
        let host = b"127.0.0.1\0";
        let server = sengoo_http_server_bind(host.as_ptr(), 0);
        assert!(server != 0);
        let port = sengoo_http_server_local_port(server) as u16;

        let first_client = thread::spawn(move || {
            let request = b"GET /drain/a HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n";
            send_raw_http_request(port, request)
        });
        let first = sengoo_http_server_next_request(server, 4_000);
        assert!(first != 0);
        let second_client = thread::spawn(move || {
            let request = b"GET /drain/b HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n";
            send_raw_http_request(port, request)
        });
        let second = sengoo_http_server_next_request(server, 4_000);
        assert!(second != 0);

        assert_eq!(sengoo_http_server_close(server), 1);

        let (first_status, first_body) =
            parse_http_status_and_body(&first_client.join().expect("first client"));
        assert_eq!(first_status, 504);
        assert_eq!(first_body, b"gateway timeout");
        let (second_status, second_body) =
            parse_http_status_and_body(&second_client.join().expect("second client"));
        assert_eq!(second_status, 504);
        assert_eq!(second_body, b"gateway timeout");

        assert_eq!(
            net_runtime()
                .http_request_pending_count(server)
                .expect("pending count"),
            0,
            "drained server must leave no request entries behind"
        );
        assert_eq!(
            net_runtime().http_request_table_len(),
            0,
            "drained server must leave the request handle table empty"
        );
        assert_eq!(
            sengoo_http_request_respond(first, 200, b"x".as_ptr(), 1),
            0,
            "drained handles must be invalid"
        );
        assert_eq!(
            sengoo_net_last_error(),
            i64::from(SENGOO_NET_ERR_HANDLE_NOT_FOUND)
        );
    }
}
