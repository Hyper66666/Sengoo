use std::collections::HashMap;
use std::io::ErrorKind;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;
use std::time::{Duration, Instant};

use super::websocket::{
    run_ws_echo_session, websocket_accept_value, write_websocket_upgrade_response,
};
use super::{
    classify_io_error, connect_timeout, decode_chunked_body, fail_bool, fail_handle, fail_i64,
    net_runtime, parse_host, reset_last_error, set_last_error, NetErrorCode, NetRuntime,
};

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
pub(super) enum HttpServerRouteKind {
    StaticResponse { status: i32, body: Vec<u8> },
    WebSocketEcho,
}

#[derive(Debug, Clone)]
pub(super) struct HttpServerRoute {
    pub(super) method: String,
    pub(super) path_pattern: String,
    pub(super) kind: HttpServerRouteKind,
}

#[derive(Debug, Clone)]
pub(super) enum HttpServerMiddlewareKind {
    RequireHeader {
        name: String,
        value: String,
        reject_status: i32,
        reject_body: Vec<u8>,
    },
}

#[derive(Debug, Clone)]
pub(super) struct HttpServerMiddleware {
    pub(super) kind: HttpServerMiddlewareKind,
}

#[derive(Debug)]
pub(super) struct HttpServerState {
    pub(super) listener: TcpListener,
    pub(super) routes: Vec<HttpServerRoute>,
    pub(super) middlewares: Vec<HttpServerMiddleware>,
    pub(super) max_header_bytes: usize,
    pub(super) max_body_bytes: usize,
}

type HttpServerSnapshot = (
    TcpListener,
    Vec<HttpServerRoute>,
    Vec<HttpServerMiddleware>,
    usize,
    usize,
);

impl NetRuntime {
    pub(crate) fn http_server_store(&self, state: HttpServerState) -> Result<u64, NetErrorCode> {
        let handle = self.alloc_handle();
        self.http_servers
            .lock()
            .map_err(|_| NetErrorCode::InternalError)?
            .insert(handle, state);
        Ok(handle)
    }

    pub(crate) fn http_server_local_port(&self, handle: u64) -> Result<i64, NetErrorCode> {
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

    pub(crate) fn http_server_set_limits(
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

    pub(crate) fn http_server_add_route(
        &self,
        handle: u64,
        route: HttpServerRoute,
    ) -> Result<i64, NetErrorCode> {
        self.http_server_with_state(handle, |state| {
            state.routes.push(route);
            1
        })
    }

    pub(crate) fn http_server_add_middleware(
        &self,
        handle: u64,
        middleware: HttpServerMiddleware,
    ) -> Result<i64, NetErrorCode> {
        self.http_server_with_state(handle, |state| {
            state.middlewares.push(middleware);
            1
        })
    }

    pub(crate) fn http_server_snapshot(
        &self,
        handle: u64,
    ) -> Result<HttpServerSnapshot, NetErrorCode> {
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

    pub(crate) fn http_server_close(&self, handle: u64) -> Result<i64, NetErrorCode> {
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
