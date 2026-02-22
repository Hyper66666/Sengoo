use std::collections::HashMap;
use std::io::ErrorKind;
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs, UdpSocket};
use std::sync::atomic::{AtomicI32, AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

static NEXT_NET_HANDLE: AtomicU64 = AtomicU64::new(1);
static LAST_NET_ERROR: AtomicI32 = AtomicI32::new(0);
static TCP_STREAMS: OnceLock<Mutex<HashMap<u64, TcpStream>>> = OnceLock::new();
static UDP_SOCKETS: OnceLock<Mutex<HashMap<u64, UdpSocket>>> = OnceLock::new();
static HTTP_RESPONSES: OnceLock<Mutex<HashMap<u64, HttpResponseEntry>>> = OnceLock::new();
static WS_STREAMS: OnceLock<Mutex<HashMap<u64, TcpStream>>> = OnceLock::new();

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

fn tcp_streams() -> &'static Mutex<HashMap<u64, TcpStream>> {
    TCP_STREAMS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn udp_sockets() -> &'static Mutex<HashMap<u64, UdpSocket>> {
    UDP_SOCKETS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn http_responses() -> &'static Mutex<HashMap<u64, HttpResponseEntry>> {
    HTTP_RESPONSES.get_or_init(|| Mutex::new(HashMap::new()))
}

fn ws_streams() -> &'static Mutex<HashMap<u64, TcpStream>> {
    WS_STREAMS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn next_handle() -> u64 {
    NEXT_NET_HANDLE.fetch_add(1, Ordering::Relaxed)
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
        let default_port = if scheme.eq_ignore_ascii_case("https") {
            443
        } else if scheme.eq_ignore_ascii_case("wss") {
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
    let addr = match parse_addr(host, port) {
        Ok(addr) => addr,
        Err(code) => return fail_handle(code),
    };
    let mut addrs = match addr.to_socket_addrs() {
        Ok(addrs) => addrs,
        Err(_) => return fail_handle(NetErrorCode::ResolveFailed),
    };
    let first_addr = match addrs.next() {
        Some(addr) => addr,
        None => return fail_handle(NetErrorCode::ResolveFailed),
    };
    let stream = match TcpStream::connect_timeout(&first_addr, connect_timeout(timeout_ms)) {
        Ok(stream) => stream,
        Err(err) => {
            if err.kind() == ErrorKind::TimedOut {
                return fail_handle(NetErrorCode::Timeout);
            }
            return fail_handle(NetErrorCode::ConnectFailed);
        }
    };
    if let Err(err) = stream.set_nodelay(true) {
        return fail_handle(classify_io_error(&err));
    }
    if let Err(err) = stream.set_read_timeout(Some(connect_timeout(timeout_ms))) {
        return fail_handle(classify_io_error(&err));
    }
    if let Err(err) = stream.set_write_timeout(Some(connect_timeout(timeout_ms))) {
        return fail_handle(classify_io_error(&err));
    }

    let handle = next_handle();
    match tcp_streams().lock() {
        Ok(mut table) => {
            table.insert(handle, stream);
            handle
        }
        Err(_) => fail_handle(NetErrorCode::InternalError),
    }
}

#[no_mangle]
pub extern "C" fn sengoo_tcp_send(handle: u64, data: *const u8, len: usize) -> i64 {
    reset_last_error();
    if data.is_null() {
        return fail_i64(NetErrorCode::InvalidArgument);
    }
    let Ok(mut table) = tcp_streams().lock() else {
        return fail_i64(NetErrorCode::InternalError);
    };
    let Some(stream) = table.get_mut(&handle) else {
        return fail_i64(NetErrorCode::HandleNotFound);
    };
    let payload = unsafe { std::slice::from_raw_parts(data, len) };
    match stream.write(payload) {
        Ok(n) => n as i64,
        Err(err) => fail_i64(classify_io_error(&err)),
    }
}

#[no_mangle]
pub extern "C" fn sengoo_tcp_recv(
    handle: u64,
    buffer: *mut u8,
    capacity: usize,
    timeout_ms: u32,
) -> i64 {
    reset_last_error();
    if buffer.is_null() {
        return fail_i64(NetErrorCode::InvalidArgument);
    }
    let Ok(mut table) = tcp_streams().lock() else {
        return fail_i64(NetErrorCode::InternalError);
    };
    let Some(stream) = table.get_mut(&handle) else {
        return fail_i64(NetErrorCode::HandleNotFound);
    };

    if timeout_ms != 0 {
        if let Err(err) = stream.set_read_timeout(Some(Duration::from_millis(timeout_ms as u64))) {
            return fail_i64(classify_io_error(&err));
        }
    }

    let target = unsafe { std::slice::from_raw_parts_mut(buffer, capacity) };
    match stream.read(target) {
        Ok(n) => n as i64,
        Err(err) if err.kind() == ErrorKind::WouldBlock => {
            set_last_error(NetErrorCode::Timeout);
            0
        }
        Err(err) if err.kind() == ErrorKind::TimedOut => {
            set_last_error(NetErrorCode::Timeout);
            0
        }
        Err(err) => fail_i64(classify_io_error(&err)),
    }
}

#[no_mangle]
pub extern "C" fn sengoo_tcp_close(handle: u64) -> i64 {
    reset_last_error();
    let Ok(mut table) = tcp_streams().lock() else {
        return fail_bool(NetErrorCode::InternalError);
    };
    if table.remove(&handle).is_some() {
        1
    } else {
        fail_bool(NetErrorCode::HandleNotFound)
    }
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
    let addr = format!("{}:{}", host, port);
    let socket = match UdpSocket::bind(addr) {
        Ok(socket) => socket,
        Err(err) => return fail_handle(classify_io_error(&err)),
    };
    if let Err(err) = socket.set_nonblocking(false) {
        return fail_handle(classify_io_error(&err));
    }

    let handle = next_handle();
    match udp_sockets().lock() {
        Ok(mut table) => {
            table.insert(handle, socket);
            handle
        }
        Err(_) => fail_handle(NetErrorCode::InternalError),
    }
}

#[no_mangle]
pub extern "C" fn sengoo_udp_connect(handle: u64, host: *const u8, port: u16) -> i64 {
    reset_last_error();
    let addr = match parse_addr(host, port) {
        Ok(addr) => addr,
        Err(code) => return fail_bool(code),
    };
    let Ok(mut table) = udp_sockets().lock() else {
        return fail_bool(NetErrorCode::InternalError);
    };
    let Some(socket) = table.get_mut(&handle) else {
        return fail_bool(NetErrorCode::HandleNotFound);
    };
    if socket.connect(addr).is_ok() {
        1
    } else {
        fail_bool(NetErrorCode::ConnectFailed)
    }
}

#[no_mangle]
pub extern "C" fn sengoo_udp_send(handle: u64, data: *const u8, len: usize) -> i64 {
    reset_last_error();
    if data.is_null() {
        return fail_i64(NetErrorCode::InvalidArgument);
    }
    let Ok(mut table) = udp_sockets().lock() else {
        return fail_i64(NetErrorCode::InternalError);
    };
    let Some(socket) = table.get_mut(&handle) else {
        return fail_i64(NetErrorCode::HandleNotFound);
    };
    let payload = unsafe { std::slice::from_raw_parts(data, len) };
    match socket.send(payload) {
        Ok(n) => n as i64,
        Err(err) => fail_i64(classify_io_error(&err)),
    }
}

#[no_mangle]
pub extern "C" fn sengoo_udp_recv(
    handle: u64,
    buffer: *mut u8,
    capacity: usize,
    timeout_ms: u32,
) -> i64 {
    reset_last_error();
    if buffer.is_null() {
        return fail_i64(NetErrorCode::InvalidArgument);
    }
    let Ok(mut table) = udp_sockets().lock() else {
        return fail_i64(NetErrorCode::InternalError);
    };
    let Some(socket) = table.get_mut(&handle) else {
        return fail_i64(NetErrorCode::HandleNotFound);
    };
    if timeout_ms != 0 {
        if let Err(err) = socket.set_read_timeout(Some(Duration::from_millis(timeout_ms as u64))) {
            return fail_i64(classify_io_error(&err));
        }
    }

    let target = unsafe { std::slice::from_raw_parts_mut(buffer, capacity) };
    match socket.recv(target) {
        Ok(n) => n as i64,
        Err(err) if err.kind() == ErrorKind::WouldBlock => {
            set_last_error(NetErrorCode::Timeout);
            0
        }
        Err(err) if err.kind() == ErrorKind::TimedOut => {
            set_last_error(NetErrorCode::Timeout);
            0
        }
        Err(err) => fail_i64(classify_io_error(&err)),
    }
}

#[no_mangle]
pub extern "C" fn sengoo_udp_close(handle: u64) -> i64 {
    reset_last_error();
    let Ok(mut table) = udp_sockets().lock() else {
        return fail_bool(NetErrorCode::InternalError);
    };
    if table.remove(&handle).is_some() {
        1
    } else {
        fail_bool(NetErrorCode::HandleNotFound)
    }
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
    let handle = next_handle();
    if let Ok(mut table) = http_responses().lock() {
        table.insert(handle, response);
        handle
    } else {
        fail_handle(NetErrorCode::InternalError)
    }
}

#[no_mangle]
pub extern "C" fn sengoo_http_post(
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
    let handle = next_handle();
    if let Ok(mut table) = http_responses().lock() {
        table.insert(handle, response);
        handle
    } else {
        fail_handle(NetErrorCode::InternalError)
    }
}

#[no_mangle]
pub extern "C" fn sengoo_http_status(handle: u64) -> i64 {
    reset_last_error();
    let Ok(table) = http_responses().lock() else {
        return fail_i64(NetErrorCode::InternalError);
    };
    match table.get(&handle) {
        Some(resp) => resp.status_code,
        None => fail_i64(NetErrorCode::HandleNotFound),
    }
}

#[no_mangle]
pub extern "C" fn sengoo_http_body_len(handle: u64) -> i64 {
    reset_last_error();
    let Ok(table) = http_responses().lock() else {
        return fail_i64(NetErrorCode::InternalError);
    };
    match table.get(&handle) {
        Some(resp) => resp.body.len() as i64,
        None => fail_i64(NetErrorCode::HandleNotFound),
    }
}

#[no_mangle]
pub extern "C" fn sengoo_http_body_copy(handle: u64, buffer: *mut u8, capacity: usize) -> i64 {
    reset_last_error();
    if buffer.is_null() {
        return fail_i64(NetErrorCode::InvalidArgument);
    }
    let Ok(table) = http_responses().lock() else {
        return fail_i64(NetErrorCode::InternalError);
    };
    let Some(response) = table.get(&handle) else {
        return fail_i64(NetErrorCode::HandleNotFound);
    };
    copy_bytes_to_buffer(&response.body, buffer, capacity)
}

#[no_mangle]
pub extern "C" fn sengoo_http_close(handle: u64) -> i64 {
    reset_last_error();
    let Ok(mut table) = http_responses().lock() else {
        return fail_bool(NetErrorCode::InternalError);
    };
    if table.remove(&handle).is_some() {
        1
    } else {
        fail_bool(NetErrorCode::HandleNotFound)
    }
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
    let handle = next_handle();
    if let Ok(mut table) = ws_streams().lock() {
        table.insert(handle, stream);
        handle
    } else {
        fail_handle(NetErrorCode::InternalError)
    }
}

#[no_mangle]
pub extern "C" fn sengoo_ws_send_text(handle: u64, data: *const u8, len: usize) -> i64 {
    reset_last_error();
    if data.is_null() {
        return fail_i64(NetErrorCode::InvalidArgument);
    }
    let payload = unsafe { std::slice::from_raw_parts(data, len) };
    let Ok(mut table) = ws_streams().lock() else {
        return fail_i64(NetErrorCode::InternalError);
    };
    let Some(stream) = table.get_mut(&handle) else {
        return fail_i64(NetErrorCode::HandleNotFound);
    };
    match ws_write_frame(stream, 0x1, payload, true) {
        Ok(_) => len as i64,
        Err(err) => fail_i64(classify_io_error(&err)),
    }
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
    let Ok(mut table) = ws_streams().lock() else {
        return fail_i64(NetErrorCode::InternalError);
    };
    let Some(stream) = table.get_mut(&handle) else {
        return fail_i64(NetErrorCode::HandleNotFound);
    };
    if timeout_ms != 0 {
        if let Err(err) = stream.set_read_timeout(Some(Duration::from_millis(timeout_ms as u64))) {
            return fail_i64(classify_io_error(&err));
        }
    }

    loop {
        let Some((opcode, payload)) = ws_read_frame(stream) else {
            return fail_i64(NetErrorCode::WebSocketProtocolError);
        };
        match opcode {
            0x1 => return copy_bytes_to_buffer(&payload, buffer, capacity),
            0x9 => {
                // Ping -> pong
                if let Err(err) = ws_write_frame(stream, 0xA, &payload, true) {
                    return fail_i64(classify_io_error(&err));
                }
            }
            0x8 => {
                set_last_error(NetErrorCode::RemoteClosed);
                return 0;
            }
            0xA => {}
            _ => return fail_i64(NetErrorCode::WebSocketProtocolError),
        }
    }
}

#[no_mangle]
pub extern "C" fn sengoo_ws_close(handle: u64) -> i64 {
    reset_last_error();
    let Ok(mut table) = ws_streams().lock() else {
        return fail_bool(NetErrorCode::InternalError);
    };
    let Some(mut stream) = table.remove(&handle) else {
        return fail_bool(NetErrorCode::HandleNotFound);
    };
    if let Err(err) = ws_write_frame(&mut stream, 0x8, &[], true) {
        return fail_bool(classify_io_error(&err));
    }
    1
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
        let sent = sengoo_tcp_send(handle, msg.as_ptr(), msg.len());
        assert_eq!(sent, msg.len() as i64, "tcp send should send payload");

        let mut out = [0u8; 16];
        let received = sengoo_tcp_recv(handle, out.as_mut_ptr(), out.len(), 2_000);
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
        let sent = sengoo_udp_send(handle, msg.as_ptr(), msg.len());
        assert_eq!(sent, msg.len() as i64, "udp send should send payload");

        let mut out = [0u8; 16];
        let received = sengoo_udp_recv(handle, out.as_mut_ptr(), out.len(), 2_000);
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
        assert_eq!(sengoo_ws_send_text(handle, msg.as_ptr(), msg.len()), 4);

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
    fn invalid_argument_sets_error_mapping() {
        let ret = sengoo_tcp_send(999, std::ptr::null(), 5);
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
}
