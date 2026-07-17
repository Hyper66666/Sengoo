use std::collections::HashMap;
use std::io::ErrorKind;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::ptr::NonNull;
use std::sync::atomic::{AtomicU8, Ordering};
#[cfg(not(windows))]
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

#[cfg(windows)]
use native_tls::{Identity, TlsAcceptor};

#[cfg(not(windows))]
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
#[cfg(not(windows))]
use rustls::{ServerConfig, ServerConnection, StreamOwned};

use super::websocket::{
    run_ws_echo_session, websocket_accept_value, write_websocket_upgrade_response,
};
use super::{
    classify_io_error, connect_timeout, decode_chunked_body, fail_bool, fail_handle, fail_i64,
    net_runtime, parse_host, reset_last_error, set_last_error, NetErrorCode, NetRuntime,
};

/// Accepted server-side connection: plaintext TCP or TLS over TCP.
#[derive(Debug)]
pub(super) enum HttpServerConn {
    Plain(TcpStream),
    #[cfg(windows)]
    Tls(Box<native_tls::TlsStream<TcpStream>>),
    #[cfg(not(windows))]
    Tls(Box<StreamOwned<ServerConnection, TcpStream>>),
}

impl Read for HttpServerConn {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            HttpServerConn::Plain(s) => s.read(buf),
            #[cfg(windows)]
            HttpServerConn::Tls(s) => s.read(buf),
            #[cfg(not(windows))]
            HttpServerConn::Tls(s) => s.read(buf),
        }
    }
}

impl Write for HttpServerConn {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self {
            HttpServerConn::Plain(s) => s.write(buf),
            #[cfg(windows)]
            HttpServerConn::Tls(s) => s.write(buf),
            #[cfg(not(windows))]
            HttpServerConn::Tls(s) => s.write(buf),
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            HttpServerConn::Plain(s) => s.flush(),
            #[cfg(windows)]
            HttpServerConn::Tls(s) => s.flush(),
            #[cfg(not(windows))]
            HttpServerConn::Tls(s) => s.flush(),
        }
    }
}

impl HttpServerConn {
    fn set_read_timeout(&self, timeout: Option<Duration>) -> std::io::Result<()> {
        match self {
            HttpServerConn::Plain(s) => s.set_read_timeout(timeout),
            #[cfg(windows)]
            HttpServerConn::Tls(s) => s.get_ref().set_read_timeout(timeout),
            #[cfg(not(windows))]
            HttpServerConn::Tls(s) => s.sock.set_read_timeout(timeout),
        }
    }

    fn set_write_timeout(&self, timeout: Option<Duration>) -> std::io::Result<()> {
        match self {
            HttpServerConn::Plain(s) => s.set_write_timeout(timeout),
            #[cfg(windows)]
            HttpServerConn::Tls(s) => s.get_ref().set_write_timeout(timeout),
            #[cfg(not(windows))]
            HttpServerConn::Tls(s) => s.sock.set_write_timeout(timeout),
        }
    }
}

/// Platform TLS acceptor built from PEM cert chain + PKCS#8 PEM key.
#[derive(Clone)]
pub(super) enum HttpTlsAcceptor {
    #[cfg(windows)]
    Native(TlsAcceptor),
    #[cfg(not(windows))]
    Rustls(Arc<ServerConfig>),
}

impl std::fmt::Debug for HttpTlsAcceptor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            #[cfg(windows)]
            HttpTlsAcceptor::Native(_) => f.write_str("HttpTlsAcceptor::Native"),
            #[cfg(not(windows))]
            HttpTlsAcceptor::Rustls(_) => f.write_str("HttpTlsAcceptor::Rustls"),
        }
    }
}

impl HttpTlsAcceptor {
    fn accept(&self, stream: TcpStream) -> Result<HttpServerConn, NetErrorCode> {
        match self {
            #[cfg(windows)]
            HttpTlsAcceptor::Native(acc) => {
                let tls = acc
                    .accept(stream)
                    .map_err(|err| classify_server_tls_error(&err.to_string()))?;
                Ok(HttpServerConn::Tls(Box::new(tls)))
            }
            #[cfg(not(windows))]
            HttpTlsAcceptor::Rustls(config) => {
                let conn = ServerConnection::new(Arc::clone(config))
                    .map_err(|_| NetErrorCode::TlsUnavailable)?;
                let mut tls = StreamOwned::new(conn, stream);
                while tls.conn.is_handshaking() {
                    tls.conn
                        .complete_io(&mut tls.sock)
                        .map_err(|err| classify_server_tls_io(&err))?;
                }
                Ok(HttpServerConn::Tls(Box::new(tls)))
            }
        }
    }
}

fn classify_server_tls_error(message: &str) -> NetErrorCode {
    let lower = message.to_ascii_lowercase();
    if lower.contains("cert")
        || lower.contains("certificate")
        || lower.contains("pkcs")
        || lower.contains("key")
        || lower.contains("pem")
        || lower.contains("identity")
        || lower.contains("import")
        || lower.contains("invalid")
        || lower.contains("empty chain")
        || lower.contains("not a pkcs")
    {
        NetErrorCode::TlsCertInvalid
    } else if lower.contains("unavail") || lower.contains("not support") {
        NetErrorCode::TlsUnavailable
    } else {
        NetErrorCode::TlsHandshake
    }
}

#[cfg(not(windows))]
fn classify_server_tls_io(err: &std::io::Error) -> NetErrorCode {
    if let Some(src) = err
        .get_ref()
        .and_then(|s| s.downcast_ref::<rustls::Error>())
    {
        return match src {
            rustls::Error::InvalidCertificate(_) => NetErrorCode::TlsCertInvalid,
            _ => NetErrorCode::TlsHandshake,
        };
    }
    classify_io_error(err)
}

/// Decode PEM blocks into raw DER payloads (no new dependencies).
#[cfg(not(windows))]
fn decode_pem_blocks(pem: &[u8]) -> Result<Vec<(String, Vec<u8>)>, NetErrorCode> {
    let text = std::str::from_utf8(pem).map_err(|_| NetErrorCode::TlsCertInvalid)?;
    let mut out = Vec::new();
    let mut lines = text.lines();
    while let Some(line) = lines.next() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("-----BEGIN ") {
            let label = rest
                .strip_suffix("-----")
                .ok_or(NetErrorCode::TlsCertInvalid)?
                .to_string();
            let mut b64 = String::new();
            for body in lines.by_ref() {
                let body = body.trim();
                if body.starts_with("-----END ") {
                    break;
                }
                b64.push_str(body);
            }
            let der = base64_decode(b64.as_bytes()).map_err(|_| NetErrorCode::TlsCertInvalid)?;
            out.push((label, der));
        }
    }
    if out.is_empty() {
        return Err(NetErrorCode::TlsCertInvalid);
    }
    Ok(out)
}

#[cfg(not(windows))]
fn base64_decode(input: &[u8]) -> Result<Vec<u8>, ()> {
    const TABLE: &[u8; 128] = &{
        let mut t = [0xffu8; 128];
        let mut i = 0u8;
        while i < 26 {
            t[(b'A' + i) as usize] = i;
            t[(b'a' + i) as usize] = 26 + i;
            i += 1;
        }
        i = 0;
        while i < 10 {
            t[(b'0' + i) as usize] = 52 + i;
            i += 1;
        }
        t[b'+' as usize] = 62;
        t[b'/' as usize] = 63;
        t
    };
    let filtered: Vec<u8> = input
        .iter()
        .copied()
        .filter(|b| !b.is_ascii_whitespace())
        .collect();
    if filtered.len() % 4 != 0 {
        return Err(());
    }
    let mut out = Vec::with_capacity(filtered.len() / 4 * 3);
    for chunk in filtered.chunks(4) {
        let mut vals = [0u8; 4];
        let mut pad = 0;
        for (i, &c) in chunk.iter().enumerate() {
            if c == b'=' {
                vals[i] = 0;
                pad += 1;
            } else if (c as usize) < 128 && TABLE[c as usize] != 0xff {
                vals[i] = TABLE[c as usize];
            } else {
                return Err(());
            }
        }
        out.push((vals[0] << 2) | (vals[1] >> 4));
        if pad < 2 {
            out.push((vals[1] << 4) | (vals[2] >> 2));
        }
        if pad < 1 {
            out.push((vals[2] << 6) | vals[3]);
        }
    }
    Ok(out)
}

fn build_tls_acceptor(cert_pem: &[u8], key_pem: &[u8]) -> Result<HttpTlsAcceptor, NetErrorCode> {
    if cert_pem.is_empty() || key_pem.is_empty() {
        return Err(NetErrorCode::InvalidArgument);
    }

    #[cfg(windows)]
    {
        let identity = Identity::from_pkcs8(cert_pem, key_pem)
            .map_err(|err| classify_server_tls_error(&err.to_string()))?;
        let acceptor = TlsAcceptor::new(identity)
            .map_err(|err| classify_server_tls_error(&err.to_string()))?;
        Ok(HttpTlsAcceptor::Native(acceptor))
    }

    #[cfg(not(windows))]
    {
        let cert_blocks = decode_pem_blocks(cert_pem)?;
        let key_blocks = decode_pem_blocks(key_pem)?;
        let certs: Vec<CertificateDer<'static>> = cert_blocks
            .into_iter()
            .filter(|(label, _)| label.contains("CERTIFICATE"))
            .map(|(_, der)| CertificateDer::from(der))
            .collect();
        if certs.is_empty() {
            return Err(NetErrorCode::TlsCertInvalid);
        }
        let key_der = key_blocks
            .into_iter()
            .find(|(label, _)| label.contains("PRIVATE KEY") && !label.contains("ENCRYPTED"))
            .map(|(_, der)| der)
            .ok_or(NetErrorCode::TlsCertInvalid)?;
        let key = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(key_der));
        let config = ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(certs, key)
            .map_err(|_| NetErrorCode::TlsCertInvalid)?;
        Ok(HttpTlsAcceptor::Rustls(Arc::new(config)))
    }
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

/// Serve-loop mode: unset until first pull or router claim (one-mode-per-listener).
pub(super) const HTTP_SERVER_MODE_UNSET: u8 = 0;
pub(super) const HTTP_SERVER_MODE_PULL: u8 = 1;
pub(super) const HTTP_SERVER_MODE_ROUTER: u8 = 2;

/// Keep-alive bounds (design D-C2). Applied only when keep-alive is opted in.
pub(super) const HTTP_KEEP_ALIVE_MAX_REQUESTS: u32 = 100;
pub(super) const HTTP_KEEP_ALIVE_IDLE_TIMEOUT_MS: u32 = 30_000;

/// Server-owned live connection waiting for the next request after a keep-alive
/// response. Not tied to any request handle.
#[derive(Debug)]
pub(super) struct LiveHttpConnection {
    stream: HttpServerConn,
    /// Number of fully answered requests already completed on this connection.
    answered_count: u32,
}

#[derive(Debug)]
pub(super) struct HttpServerState {
    pub(super) listener: TcpListener,
    pub(super) routes: Vec<HttpServerRoute>,
    pub(super) middlewares: Vec<HttpServerMiddleware>,
    pub(super) max_header_bytes: usize,
    pub(super) max_body_bytes: usize,
    /// 0 = unset, 1 = pull, 2 = Sengoo-side router (`serve_http`).
    pub(super) serve_mode: u8,
    /// Opt-in HTTP/1.1 connection reuse. Default false 鈬?Connection: close.
    pub(super) keep_alive_enabled: bool,
    /// Server-owned idle connection eligible for the next pull (at most one
    /// serial live connection in v1).
    pub(super) live_connection: Option<LiveHttpConnection>,
    /// When set, accepted TCP connections are TLS-wrapped before HTTP.
    pub(super) tls: Option<HttpTlsAcceptor>,
}

/// Pulled-but-unanswered dynamic request. Owns the connection until answer or
/// close; with keep-alive, a successful answer may return the stream to the
/// server-owned live connection slot instead of dropping it.
#[derive(Debug)]
pub(super) struct HttpRequestEntry {
    server_handle: u64,
    method: String,
    path: String,
    query: String,
    version: String,
    headers: HashMap<String, String>,
    body: Vec<u8>,
    stream: HttpServerConn,
    max_body_bytes: usize,
    /// Completed answers on this connection before the current request.
    answered_before: u32,
    /// Client sent `Connection: close` (or HTTP/1.0 without keep-alive).
    client_wants_close: bool,
}

/// Cap of pulled-but-unanswered request handles per server (design D5).
const MAX_PENDING_DYNAMIC_REQUESTS: usize = 64;
const ASYNC_HTTP_IO_SLICE: Duration = Duration::from_millis(5);
/// Max streaming chunk size (design D-C3).
pub(super) const HTTP_STREAM_MAX_CHUNK: usize = 65_536;

#[derive(Debug)]
enum HttpStreamBodyMode {
    /// Transfer-Encoding: chunked
    Chunked,
    /// Content-Length known; remaining bytes to write.
    Fixed { remaining: usize },
}

/// In-flight streamed response body. Request handle is consumed at begin.
#[derive(Debug)]
pub(super) struct HttpResponseStreamEntry {
    server_handle: u64,
    stream: HttpServerConn,
    mode: HttpStreamBodyMode,
    finished: bool,
    answered_before: u32,
    client_wants_close: bool,
    version: String,
}

fn gateway_timeout_response() -> HttpServerResponse {
    HttpServerResponse {
        status: 504,
        headers: Vec::new(),
        body: b"gateway timeout".to_vec(),
    }
}

type HttpServerSnapshot = (
    TcpListener,
    Vec<HttpServerRoute>,
    Vec<HttpServerMiddleware>,
    usize,
    usize,
    bool, // keep_alive_enabled
);

#[derive(Debug, Clone, Copy)]
enum DynamicRequestPoll {
    Ready(u64),
    NotReady,
}

#[derive(Debug, Clone, Copy)]
enum AsyncNextRequestOutcome {
    Pending,
    Ready { is_ok: bool, value: u64, error: i64 },
}

#[derive(Debug)]
struct AsyncNextRequestState {
    server_handle: u64,
    deadline: Instant,
    listener_interest: Option<u64>,
    lifecycle: HttpFuturePollLifecycle,
    outcome: AsyncNextRequestOutcome,
}

#[repr(C)]
pub struct HttpServerRequestHandle {
    pub handle: i64,
}

#[repr(C)]
pub struct HttpServerNextRequestResult {
    pub is_ok: bool,
    pub value: HttpServerRequestHandle,
    pub error: i64,
}

#[derive(Debug, Default)]
struct HttpFuturePollLifecycle {
    state: AtomicU8,
}

struct HttpFuturePollGuard<'a> {
    lifecycle: &'a HttpFuturePollLifecycle,
    ready: bool,
}

impl HttpFuturePollLifecycle {
    fn enter(&self) -> Result<HttpFuturePollGuard<'_>, i64> {
        match self
            .state
            .compare_exchange(0, 1, Ordering::AcqRel, Ordering::Acquire)
        {
            Ok(_) => Ok(HttpFuturePollGuard {
                lifecycle: self,
                ready: false,
            }),
            Err(1) => Err(-2),
            Err(_) => Err(-3),
        }
    }
}

impl HttpFuturePollGuard<'_> {
    fn mark_ready(mut self) {
        self.lifecycle.state.store(2, Ordering::Release);
        self.ready = true;
    }
}

impl Drop for HttpFuturePollGuard<'_> {
    fn drop(&mut self) {
        if !self.ready {
            self.lifecycle.state.store(0, Ordering::Release);
        }
    }
}

unsafe fn async_handle_mut<'a, T>(handle: i64) -> Option<&'a mut T> {
    NonNull::new(handle as *mut T).map(|mut ptr| unsafe { ptr.as_mut() })
}

unsafe fn async_handle_take_box<T>(handle: i64) -> Option<Box<T>> {
    NonNull::new(handle as *mut T).map(|ptr| unsafe { Box::from_raw(ptr.as_ptr()) })
}

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

    /// Opt in/out of HTTP/1.1 keep-alive. When disabled (default), responses
    /// always use `Connection: close` and no live connection is retained.
    pub(crate) fn http_server_set_keep_alive(
        &self,
        handle: u64,
        enabled: bool,
    ) -> Result<i64, NetErrorCode> {
        self.http_server_with_state(handle, |state| {
            state.keep_alive_enabled = enabled;
            if !enabled {
                state.live_connection = None;
            }
            1
        })
    }

    fn http_server_take_live_connection(
        &self,
        handle: u64,
    ) -> Result<Option<LiveHttpConnection>, NetErrorCode> {
        self.http_server_with_state(handle, |state| state.live_connection.take())
    }

    fn http_server_put_live_connection(
        &self,
        handle: u64,
        connection: LiveHttpConnection,
    ) -> Result<(), NetErrorCode> {
        self.http_server_with_state(handle, |state| {
            // Serial v1: replace any prior live connection (drop old).
            state.live_connection = Some(connection);
        })
    }

    fn http_server_keep_alive_enabled(&self, handle: u64) -> Result<bool, NetErrorCode> {
        self.http_server_with_state(handle, |state| state.keep_alive_enabled)
    }

    /// Wrap an accepted TCP socket: plain when no TLS acceptor, else handshake.
    fn http_server_wrap_conn(
        &self,
        handle: u64,
        stream: TcpStream,
    ) -> Result<HttpServerConn, NetErrorCode> {
        let tls = self.http_server_with_state(handle, |state| state.tls.clone())?;
        match tls {
            None => Ok(HttpServerConn::Plain(stream)),
            Some(acc) => acc.accept(stream),
        }
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

    /// Claim exclusive pull (1) or router (2) mode. Same mode is idempotent.
    pub(crate) fn http_server_claim_serve_mode(
        &self,
        handle: u64,
        mode: u8,
    ) -> Result<i64, NetErrorCode> {
        self.http_server_with_state(handle, |state| {
            if state.serve_mode == HTTP_SERVER_MODE_UNSET {
                state.serve_mode = mode;
                1
            } else if state.serve_mode == mode {
                1
            } else {
                // Signal mode conflict via InvalidArgument mapping to STATUS_INVALID_ARGUMENT.
                0
            }
        })
        .and_then(|ok| {
            if ok == 1 {
                Ok(1)
            } else {
                Err(NetErrorCode::InvalidArgument)
            }
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
        // On Windows, duplicated sockets do not inherit the non-blocking flag,
        // which would turn the accept loop into an unbounded block.
        listener
            .set_nonblocking(true)
            .map_err(|err| classify_io_error(&err))?;
        Ok((
            listener,
            state.routes.clone(),
            state.middlewares.clone(),
            state.max_header_bytes,
            state.max_body_bytes,
            state.keep_alive_enabled,
        ))
    }

    pub(crate) fn http_server_close(&self, handle: u64) -> Result<i64, NetErrorCode> {
        let mut removed_state = self
            .http_servers
            .lock()
            .map_err(|_| NetErrorCode::InternalError)?
            .remove(&handle);
        if removed_state.is_none() {
            return Err(NetErrorCode::HandleNotFound);
        }
        // Drop live keep-alive connection with the server.
        if let Some(state) = removed_state.as_mut() {
            state.live_connection = None;
        }
        for mut entry in self.http_request_drain_for_server(handle)? {
            let _ = write_http_response(
                &mut entry.stream,
                &gateway_timeout_response(),
                /*keep_alive=*/ false,
            );
        }
        // Abort any in-flight response streams (Drop closes the TCP stream).
        let _ = self.http_response_stream_drain_for_server(handle)?;
        Ok(1)
    }

    fn http_request_store(&self, entry: HttpRequestEntry) -> Result<u64, NetErrorCode> {
        let handle = self.alloc_handle();
        self.http_requests
            .lock()
            .map_err(|_| NetErrorCode::InternalError)?
            .insert(handle, entry);
        Ok(handle)
    }

    pub(super) fn http_request_pending_count(
        &self,
        server_handle: u64,
    ) -> Result<usize, NetErrorCode> {
        let table = self
            .http_requests
            .lock()
            .map_err(|_| NetErrorCode::InternalError)?;
        Ok(table
            .values()
            .filter(|entry| entry.server_handle == server_handle)
            .count())
    }

    fn http_request_with_entry<F, R>(&self, handle: u64, f: F) -> Result<R, NetErrorCode>
    where
        F: FnOnce(&HttpRequestEntry) -> R,
    {
        let table = self
            .http_requests
            .lock()
            .map_err(|_| NetErrorCode::InternalError)?;
        let entry = table.get(&handle).ok_or(NetErrorCode::HandleNotFound)?;
        Ok(f(entry))
    }

    fn http_request_take(&self, handle: u64) -> Result<HttpRequestEntry, NetErrorCode> {
        self.http_requests
            .lock()
            .map_err(|_| NetErrorCode::InternalError)?
            .remove(&handle)
            .ok_or(NetErrorCode::HandleNotFound)
    }

    fn http_request_drain_for_server(
        &self,
        server_handle: u64,
    ) -> Result<Vec<HttpRequestEntry>, NetErrorCode> {
        let mut table = self
            .http_requests
            .lock()
            .map_err(|_| NetErrorCode::InternalError)?;
        let handles: Vec<u64> = table
            .iter()
            .filter(|(_, entry)| entry.server_handle == server_handle)
            .map(|(handle, _)| *handle)
            .collect();
        Ok(handles
            .into_iter()
            .filter_map(|handle| table.remove(&handle))
            .collect())
    }

    fn http_response_stream_store(
        &self,
        entry: HttpResponseStreamEntry,
    ) -> Result<u64, NetErrorCode> {
        let handle = self.alloc_handle();
        self.http_response_streams
            .lock()
            .map_err(|_| NetErrorCode::InternalError)?
            .insert(handle, entry);
        Ok(handle)
    }

    fn http_response_stream_with_mut<F, R>(&self, handle: u64, f: F) -> Result<R, NetErrorCode>
    where
        F: FnOnce(&mut HttpResponseStreamEntry) -> Result<R, NetErrorCode>,
    {
        let mut table = self
            .http_response_streams
            .lock()
            .map_err(|_| NetErrorCode::InternalError)?;
        let entry = table.get_mut(&handle).ok_or(NetErrorCode::HandleNotFound)?;
        f(entry)
    }

    fn http_response_stream_take(
        &self,
        handle: u64,
    ) -> Result<HttpResponseStreamEntry, NetErrorCode> {
        self.http_response_streams
            .lock()
            .map_err(|_| NetErrorCode::InternalError)?
            .remove(&handle)
            .ok_or(NetErrorCode::HandleNotFound)
    }

    fn http_response_stream_drain_for_server(
        &self,
        server_handle: u64,
    ) -> Result<Vec<HttpResponseStreamEntry>, NetErrorCode> {
        let mut table = self
            .http_response_streams
            .lock()
            .map_err(|_| NetErrorCode::InternalError)?;
        let handles: Vec<u64> = table
            .iter()
            .filter(|(_, entry)| entry.server_handle == server_handle)
            .map(|(handle, _)| *handle)
            .collect();
        Ok(handles
            .into_iter()
            .filter_map(|handle| table.remove(&handle))
            .collect())
    }

    #[cfg(test)]
    pub(super) fn http_request_table_len(&self) -> usize {
        self.http_requests
            .lock()
            .map(|table| table.len())
            .unwrap_or(usize::MAX)
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
        408 => "Request Timeout",
        426 => "Upgrade Required",
        500 => "Internal Server Error",
        503 => "Service Unavailable",
        504 => "Gateway Timeout",
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
    stream: &mut HttpServerConn,
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
    stream: &mut HttpServerConn,
    response: &HttpServerResponse,
    keep_alive: bool,
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
        headers.push((
            "Connection".to_string(),
            if keep_alive {
                "keep-alive".to_string()
            } else {
                "close".to_string()
            },
        ));
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
    stream: &mut HttpServerConn,
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
                false,
            );
            return Ok(());
        }
    };
    let _request_http_version = request.version.as_str();
    let _request_body_len = request.body.len();

    if let Some(response) = apply_middlewares(middlewares, &request) {
        write_http_response(stream, &response, false)?;
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
            false,
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
                false,
            )?;
            Ok(())
        }
        HttpServerRouteKind::WebSocketEcho => answer_ws_echo_route(stream, &request),
    }
}

fn answer_ws_echo_route(
    stream: &mut HttpServerConn,
    request: &HttpServerRequest,
) -> Result<(), NetErrorCode> {
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
            false,
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
            false,
        )?;
        return Ok(());
    }
    let accept = websocket_accept_value(client_key);
    // WebSocket productization is plain-TCP only for v0.2 (TLS WS is residual).
    match stream {
        HttpServerConn::Plain(tcp) => {
            write_websocket_upgrade_response(tcp, &accept)?;
            run_ws_echo_session(tcp)
        }
        #[cfg(windows)]
        HttpServerConn::Tls(_) => Err(NetErrorCode::UnsupportedScheme),
        #[cfg(not(windows))]
        HttpServerConn::Tls(_) => Err(NetErrorCode::UnsupportedScheme),
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
            Ok((stream, _)) => {
                // Accepted sockets can inherit the listener's non-blocking
                // mode (Windows); restore blocking so read/write timeouts
                // apply instead of spurious WouldBlock protocol failures.
                stream
                    .set_nonblocking(false)
                    .map_err(|err| classify_io_error(&err))?;
                return Ok(Some(stream));
            }
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

fn try_accept_nonblocking(listener: &TcpListener) -> Result<Option<TcpStream>, NetErrorCode> {
    match listener.accept() {
        Ok((stream, _)) => {
            stream
                .set_nonblocking(false)
                .map_err(|err| classify_io_error(&err))?;
            Ok(Some(stream))
        }
        Err(err) if err.kind() == ErrorKind::WouldBlock => Ok(None),
        Err(err) => Err(classify_io_error(&err)),
    }
}

fn client_wants_connection_close(request: &HttpServerRequest) -> bool {
    if let Some(connection) = request.headers.get("connection") {
        let lower = connection.to_ascii_lowercase();
        if lower.split(',').any(|part| part.trim() == "close") {
            return true;
        }
        if lower.split(',').any(|part| part.trim() == "keep-alive") {
            return false;
        }
    }
    // HTTP/1.0 defaults to close; HTTP/1.1 defaults to keep-alive.
    !request.version.to_ascii_uppercase().starts_with("HTTP/1.1")
}

fn poll_next_dynamic_request_once(
    server_handle: u64,
    listener: &TcpListener,
    routes: &[HttpServerRoute],
    middlewares: &[HttpServerMiddleware],
    max_header_bytes: usize,
    max_body_bytes: usize,
    io_timeout: Duration,
) -> Result<DynamicRequestPoll, NetErrorCode> {
    // Prefer a server-owned live keep-alive connection before accepting.
    if let Some(live) = net_runtime().http_server_take_live_connection(server_handle)? {
        let idle_timeout =
            Duration::from_millis(u64::from(HTTP_KEEP_ALIVE_IDLE_TIMEOUT_MS)).min(io_timeout);
        match process_dynamic_request_stream(
            server_handle,
            live.stream,
            routes,
            middlewares,
            max_header_bytes,
            max_body_bytes,
            idle_timeout,
            live.answered_count,
        ) {
            Ok(DynamicRequestPoll::Ready(handle)) => return Ok(DynamicRequestPoll::Ready(handle)),
            Ok(DynamicRequestPoll::NotReady) => {
                // Live connection had no usable dynamic request (idle/error/static).
                // Fall through to accept a fresh client.
            }
            Err(NetErrorCode::Timeout) => {
                // Idle timeout: drop live connection and accept new.
            }
            Err(code) => return Err(code),
        }
    }

    let Some(tcp) = try_accept_nonblocking(listener)? else {
        return Ok(DynamicRequestPoll::NotReady);
    };
    let stream = net_runtime().http_server_wrap_conn(server_handle, tcp)?;

    process_dynamic_request_stream(
        server_handle,
        stream,
        routes,
        middlewares,
        max_header_bytes,
        max_body_bytes,
        io_timeout,
        /*answered_before=*/ 0,
    )
}

#[allow(clippy::too_many_arguments)]
fn process_dynamic_request_stream(
    server_handle: u64,
    mut stream: HttpServerConn,
    routes: &[HttpServerRoute],
    middlewares: &[HttpServerMiddleware],
    max_header_bytes: usize,
    max_body_bytes: usize,
    io_timeout: Duration,
    answered_before: u32,
) -> Result<DynamicRequestPoll, NetErrorCode> {
    stream
        .set_read_timeout(Some(io_timeout))
        .map_err(|err| classify_io_error(&err))?;
    stream
        .set_write_timeout(Some(io_timeout))
        .map_err(|err| classify_io_error(&err))?;

    let request = match read_http_request(&mut stream, max_header_bytes, max_body_bytes) {
        Ok(request) => request,
        Err(code) if matches!(code, NetErrorCode::Timeout) && answered_before > 0 => {
            // Idle keep-alive timeout: close quietly.
            return Err(NetErrorCode::Timeout);
        }
        Err(_) => {
            let _ = write_http_response(
                &mut stream,
                &HttpServerResponse {
                    status: 400,
                    headers: Vec::new(),
                    body: b"bad request".to_vec(),
                },
                false,
            );
            return Ok(DynamicRequestPoll::NotReady);
        }
    };

    let wants_close = client_wants_connection_close(&request);

    if let Some(response) = apply_middlewares(middlewares, &request) {
        let _ = write_http_response(&mut stream, &response, false);
        return Ok(DynamicRequestPoll::NotReady);
    }

    if let Some((route, params)) = find_route(routes, &request.method, &request.path) {
        match route.kind {
            HttpServerRouteKind::StaticResponse { status, body } => {
                let rendered = render_route_body(&body, &params);
                let _ = write_http_response(
                    &mut stream,
                    &HttpServerResponse {
                        status,
                        headers: Vec::new(),
                        body: rendered,
                    },
                    false,
                );
            }
            HttpServerRouteKind::WebSocketEcho => {
                let _ = answer_ws_echo_route(&mut stream, &request);
            }
        }
        return Ok(DynamicRequestPoll::NotReady);
    }

    let pending = net_runtime().http_request_pending_count(server_handle)?;
    if pending >= MAX_PENDING_DYNAMIC_REQUESTS {
        let _ = write_http_response(
            &mut stream,
            &HttpServerResponse {
                status: 503,
                headers: Vec::new(),
                body: b"service unavailable".to_vec(),
            },
            false,
        );
        return Ok(DynamicRequestPoll::NotReady);
    }

    let (path, query) = match request.path.split_once('?') {
        Some((path, query)) => (path.to_string(), query.to_string()),
        None => (request.path.clone(), String::new()),
    };
    let entry = HttpRequestEntry {
        server_handle,
        method: request.method,
        path,
        query,
        version: request.version,
        headers: request.headers,
        body: request.body,
        stream,
        max_body_bytes,
        answered_before,
        client_wants_close: wants_close,
    };
    net_runtime()
        .http_request_store(entry)
        .map(DynamicRequestPoll::Ready)
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
        serve_mode: HTTP_SERVER_MODE_UNSET,
        keep_alive_enabled: false,
        live_connection: None,
        tls: None,
    };
    net_runtime()
        .http_server_store(state)
        .unwrap_or_else(fail_handle)
}

/// Bind an HTTPS listener with PEM certificate chain + PKCS#8 PEM private key.
/// Uses native-tls/Schannel on Windows and rustls on POSIX. Empty/invalid PEM
/// maps to `STATUS_TLS_*` / `STATUS_INVALID_ARGUMENT` (never silent plaintext).
#[no_mangle]
pub extern "C" fn sengoo_http_server_bind_tls(
    host: *const u8,
    port: u16,
    cert_pem: *const u8,
    cert_len: usize,
    key_pem: *const u8,
    key_len: usize,
) -> u64 {
    reset_last_error();
    let host = if host.is_null() {
        "127.0.0.1".to_string()
    } else {
        match parse_host(host) {
            Ok(host) => host,
            Err(code) => return fail_handle(code),
        }
    };
    let cert = match read_c_buffer(cert_pem, cert_len) {
        Ok(v) => v,
        Err(code) => return fail_handle(code),
    };
    let key = match read_c_buffer(key_pem, key_len) {
        Ok(v) => v,
        Err(code) => return fail_handle(code),
    };
    let acceptor = match build_tls_acceptor(&cert, &key) {
        Ok(a) => a,
        Err(code) => return fail_handle(code),
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
        serve_mode: HTTP_SERVER_MODE_UNSET,
        keep_alive_enabled: false,
        live_connection: None,
        tls: Some(acceptor),
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

/// Enable (`enabled != 0`) or disable keep-alive. Bounds are fixed at
/// `HTTP_KEEP_ALIVE_MAX_REQUESTS` / `HTTP_KEEP_ALIVE_IDLE_TIMEOUT_MS`.
#[no_mangle]
pub extern "C" fn sengoo_http_server_set_keep_alive(handle: u64, enabled: i64) -> i64 {
    reset_last_error();
    net_runtime()
        .http_server_set_keep_alive(handle, enabled != 0)
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
    let (listener, routes, middlewares, max_header_bytes, max_body_bytes, _keep_alive) =
        match net_runtime().http_server_snapshot(handle) {
            Ok(snapshot) => snapshot,
            Err(code) => return fail_i64(code),
        };

    let Some(tcp) = (match accept_with_timeout(&listener, timeout_ms) {
        Ok(stream) => stream,
        Err(code) => return fail_i64(code),
    }) else {
        set_last_error(NetErrorCode::Timeout);
        return 0;
    };
    let mut stream = match net_runtime().http_server_wrap_conn(handle, tcp) {
        Ok(s) => s,
        Err(code) => return fail_i64(code),
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
    match net_runtime().http_server_close(handle) {
        Ok(value) => value,
        Err(NetErrorCode::HandleNotFound) => 1,
        Err(code) => fail_bool(code),
    }
}

/// Claim serve mode for a listener. `mode` is 1 (pull) or 2 (router).
/// Returns 1 on success, 0 on failure (`STATUS_INVALID_ARGUMENT` if already claimed differently).
#[no_mangle]
pub extern "C" fn sengoo_http_server_claim_serve_mode(handle: u64, mode: u8) -> i64 {
    reset_last_error();
    if mode != HTTP_SERVER_MODE_PULL && mode != HTTP_SERVER_MODE_ROUTER {
        return fail_bool(NetErrorCode::InvalidArgument);
    }
    net_runtime()
        .http_server_claim_serve_mode(handle, mode)
        .unwrap_or_else(fail_bool)
}

fn http_server_next_request_impl(handle: u64, timeout_ms: u32) -> u64 {
    let (listener, routes, middlewares, max_header_bytes, max_body_bytes, _keep_alive) =
        match net_runtime().http_server_snapshot(handle) {
            Ok(snapshot) => snapshot,
            Err(code) => return fail_handle(code),
        };

    let deadline = Instant::now()
        .checked_add(Duration::from_millis(timeout_ms.max(1) as u64))
        .unwrap_or_else(Instant::now);
    let io_timeout = connect_timeout(timeout_ms);

    loop {
        let now = Instant::now();
        if now >= deadline {
            set_last_error(NetErrorCode::Timeout);
            return 0;
        }

        match poll_next_dynamic_request_once(
            handle,
            &listener,
            &routes,
            &middlewares,
            max_header_bytes,
            max_body_bytes,
            io_timeout,
        ) {
            Ok(DynamicRequestPoll::Ready(request_handle)) => return request_handle,
            Ok(DynamicRequestPoll::NotReady) => {
                let remaining = deadline.saturating_duration_since(now);
                thread::sleep(remaining.min(Duration::from_millis(2)));
            }
            Err(code) => return fail_handle(code),
        }
    }
}

#[no_mangle]
pub extern "C" fn sengoo_http_server_next_request(handle: u64, timeout_ms: u32) -> u64 {
    reset_last_error();
    // Public pull API: claim pull mode (rejects if router already claimed).
    match net_runtime().http_server_claim_serve_mode(handle, HTTP_SERVER_MODE_PULL) {
        Ok(_) => {}
        Err(NetErrorCode::InvalidArgument) => return fail_handle(NetErrorCode::InvalidArgument),
        Err(code) => return fail_handle(code),
    }
    http_server_next_request_impl(handle, timeout_ms)
}

/// Pull for Sengoo-side router only (mode already claimed as ROUTER).
#[no_mangle]
pub extern "C" fn sengoo_http_server_next_request_router(handle: u64, timeout_ms: u32) -> u64 {
    reset_last_error();
    http_server_next_request_impl(handle, timeout_ms)
}

fn http_server_next_request_async_start_impl(handle: u64, timeout_ms: u32) -> i64 {
    if handle == 0 {
        return 0;
    }
    let (listener_interest, outcome) = match net_runtime().http_server_snapshot(handle) {
        Ok((listener, _, _, _, _, _)) => {
            match crate::async_runtime::http_listener_register(&listener) {
                Ok(interest) => (Some(interest), AsyncNextRequestOutcome::Pending),
                Err(error) => (
                    None,
                    AsyncNextRequestOutcome::Ready {
                        is_ok: false,
                        value: 0,
                        error: super::status_from_net_error_code(classify_io_error(
                            &std::io::Error::from(error),
                        )),
                    },
                ),
            }
        }
        Err(code) => (
            None,
            AsyncNextRequestOutcome::Ready {
                is_ok: false,
                value: 0,
                error: super::status_from_net_error_code(code),
            },
        ),
    };
    let state = AsyncNextRequestState {
        server_handle: handle,
        deadline: Instant::now()
            .checked_add(Duration::from_millis(timeout_ms.max(1) as u64))
            .unwrap_or_else(Instant::now),
        listener_interest,
        lifecycle: HttpFuturePollLifecycle::default(),
        outcome,
    };
    Box::into_raw(Box::new(state)) as i64
}

#[no_mangle]
pub extern "C" fn sengoo_http_server_next_request_async__start(
    handle: u64,
    timeout_ms: u32,
) -> i64 {
    match net_runtime().http_server_claim_serve_mode(handle, HTTP_SERVER_MODE_PULL) {
        Ok(_) => {}
        Err(code) => {
            set_last_error(code);
            return 0;
        }
    }
    http_server_next_request_async_start_impl(handle, timeout_ms)
}

/// Async pull for Sengoo-side router only (mode already claimed as ROUTER).
/// Named `*_async__start` so Future lowering matches the standard async ABI.
#[no_mangle]
pub extern "C" fn sengoo_http_server_next_request_router_async__start(
    handle: u64,
    timeout_ms: u32,
) -> i64 {
    http_server_next_request_async_start_impl(handle, timeout_ms)
}

#[no_mangle]
/// # Safety
///
/// `handle` must be zero or a live handle returned by
/// [`sengoo_http_server_next_request_async__start`].
pub unsafe extern "C" fn sengoo_http_server_next_request_async__poll(handle: i64) -> i64 {
    let Some(state) = (unsafe { async_handle_mut::<AsyncNextRequestState>(handle) }) else {
        return 1;
    };
    let guard = match state.lifecycle.enter() {
        Ok(guard) => guard,
        Err(error) => return error,
    };
    if let AsyncNextRequestOutcome::Ready { .. } = state.outcome {
        guard.mark_ready();
        return 1;
    }

    let now = Instant::now();
    if now >= state.deadline {
        if let Some(interest) = state.listener_interest.take() {
            crate::async_runtime::http_listener_unregister(interest);
        }
        state.outcome = AsyncNextRequestOutcome::Ready {
            is_ok: false,
            value: 0,
            error: super::status_from_net_error_code(NetErrorCode::Timeout),
        };
        guard.mark_ready();
        return 1;
    }

    let (listener, routes, middlewares, max_header_bytes, max_body_bytes, _keep_alive) =
        match net_runtime().http_server_snapshot(state.server_handle) {
            Ok(snapshot) => snapshot,
            Err(code) => {
                if let Some(interest) = state.listener_interest.take() {
                    crate::async_runtime::http_listener_unregister(interest);
                }
                state.outcome = AsyncNextRequestOutcome::Ready {
                    is_ok: false,
                    value: 0,
                    error: super::status_from_net_error_code(code),
                };
                guard.mark_ready();
                return 1;
            }
        };

    let io_slice =
        ASYNC_HTTP_IO_SLICE.min(state.deadline.saturating_duration_since(Instant::now()));

    // Prefer server-owned keep-alive connection before accept interest.
    if let Ok(Some(live)) = net_runtime().http_server_take_live_connection(state.server_handle) {
        let idle = Duration::from_millis(u64::from(HTTP_KEEP_ALIVE_IDLE_TIMEOUT_MS)).min(io_slice);
        match process_dynamic_request_stream(
            state.server_handle,
            live.stream,
            &routes,
            &middlewares,
            max_header_bytes,
            max_body_bytes,
            idle,
            live.answered_count,
        ) {
            Ok(DynamicRequestPoll::Ready(request_handle)) => {
                if let Some(interest) = state.listener_interest.take() {
                    crate::async_runtime::http_listener_unregister(interest);
                }
                state.outcome = AsyncNextRequestOutcome::Ready {
                    is_ok: true,
                    value: request_handle,
                    error: 0,
                };
                guard.mark_ready();
                return 1;
            }
            Ok(DynamicRequestPoll::NotReady) | Err(NetErrorCode::Timeout) => {}
            Err(code) => {
                if let Some(interest) = state.listener_interest.take() {
                    crate::async_runtime::http_listener_unregister(interest);
                }
                state.outcome = AsyncNextRequestOutcome::Ready {
                    is_ok: false,
                    value: 0,
                    error: super::status_from_net_error_code(code),
                };
                guard.mark_ready();
                return 1;
            }
        }
    }

    let Some(interest) = state.listener_interest else {
        state.outcome = AsyncNextRequestOutcome::Ready {
            is_ok: false,
            value: 0,
            error: super::status_from_net_error_code(NetErrorCode::HandleNotFound),
        };
        guard.mark_ready();
        return 1;
    };

    let stream = match crate::async_runtime::http_listener_poll_accept(interest) {
        Ok(Some(stream)) => {
            crate::async_runtime::http_listener_unregister(interest);
            state.listener_interest = None;
            stream
        }
        Ok(None) => {
            crate::async_runtime::record_external_poll_wakeup_hint(
                state
                    .deadline
                    .min(Instant::now() + Duration::from_millis(5)),
            );
            return 0;
        }
        Err(error) => {
            crate::async_runtime::http_listener_unregister(interest);
            state.listener_interest = None;
            state.outcome = AsyncNextRequestOutcome::Ready {
                is_ok: false,
                value: 0,
                error: super::status_from_net_error_code(classify_io_error(&std::io::Error::from(
                    error,
                ))),
            };
            guard.mark_ready();
            return 1;
        }
    };

    let stream = match net_runtime().http_server_wrap_conn(state.server_handle, stream) {
        Ok(s) => s,
        Err(code) => {
            if let Some(interest) = state.listener_interest.take() {
                crate::async_runtime::http_listener_unregister(interest);
            }
            state.outcome = AsyncNextRequestOutcome::Ready {
                is_ok: false,
                value: 0,
                error: super::status_from_net_error_code(code),
            };
            guard.mark_ready();
            return 1;
        }
    };
    match process_dynamic_request_stream(
        state.server_handle,
        stream,
        &routes,
        &middlewares,
        max_header_bytes,
        max_body_bytes,
        io_slice,
        /*answered_before=*/ 0,
    ) {
        Ok(DynamicRequestPoll::Ready(request_handle)) => {
            state.outcome = AsyncNextRequestOutcome::Ready {
                is_ok: true,
                value: request_handle,
                error: 0,
            };
            guard.mark_ready();
            1
        }
        Ok(DynamicRequestPoll::NotReady) => {
            match crate::async_runtime::http_listener_register(&listener) {
                Ok(interest) => {
                    state.listener_interest = Some(interest);
                    crate::async_runtime::record_external_poll_wakeup_hint(
                        state
                            .deadline
                            .min(Instant::now() + Duration::from_millis(5)),
                    );
                    0
                }
                Err(error) => {
                    state.outcome = AsyncNextRequestOutcome::Ready {
                        is_ok: false,
                        value: 0,
                        error: super::status_from_net_error_code(classify_io_error(
                            &std::io::Error::from(error),
                        )),
                    };
                    guard.mark_ready();
                    1
                }
            }
        }
        Err(code) => {
            state.outcome = AsyncNextRequestOutcome::Ready {
                is_ok: false,
                value: 0,
                error: super::status_from_net_error_code(code),
            };
            guard.mark_ready();
            1
        }
    }
}

#[no_mangle]
/// # Safety
///
/// `handle` must be zero or an unconsumed handle returned by
/// [`sengoo_http_server_next_request_async__start`].
pub unsafe extern "C" fn sengoo_http_server_next_request_async__result(
    handle: i64,
) -> HttpServerNextRequestResult {
    let Some(state) = (unsafe { async_handle_take_box::<AsyncNextRequestState>(handle) }) else {
        return HttpServerNextRequestResult {
            is_ok: false,
            value: HttpServerRequestHandle { handle: 0 },
            error: super::status_from_net_error_code(NetErrorCode::HandleNotFound),
        };
    };
    if let Some(interest) = state.listener_interest {
        crate::async_runtime::http_listener_unregister(interest);
    }
    match state.outcome {
        AsyncNextRequestOutcome::Ready {
            is_ok,
            value,
            error,
        } => HttpServerNextRequestResult {
            is_ok,
            value: HttpServerRequestHandle {
                handle: value as i64,
            },
            error,
        },
        AsyncNextRequestOutcome::Pending => HttpServerNextRequestResult {
            is_ok: false,
            value: HttpServerRequestHandle { handle: 0 },
            error: super::status_from_net_error_code(NetErrorCode::Timeout),
        },
    }
}

fn release_abandoned_async_next_request(state: &AsyncNextRequestState) {
    if let Some(interest) = state.listener_interest {
        crate::async_runtime::http_listener_unregister(interest);
    }
    if let AsyncNextRequestOutcome::Ready {
        is_ok: true, value, ..
    } = state.outcome
    {
        if let Ok(mut entry) = net_runtime().http_request_take(value) {
            let _ = write_http_response(&mut entry.stream, &gateway_timeout_response());
        }
    }
}

#[no_mangle]
/// # Safety
///
/// `handle` must be zero or an unconsumed handle returned by
/// [`sengoo_http_server_next_request_async__start`].
pub unsafe extern "C" fn sengoo_http_server_next_request_async__cancel(handle: i64) -> bool {
    let Some(state) = (unsafe { async_handle_take_box::<AsyncNextRequestState>(handle) }) else {
        return false;
    };
    release_abandoned_async_next_request(&state);
    true
}

#[no_mangle]
/// # Safety
///
/// `handle` must be zero or an unconsumed handle returned by
/// [`sengoo_http_server_next_request_async__start`].
pub unsafe extern "C" fn sengoo_http_server_next_request_async__drop(handle: i64) {
    let Some(state) = (unsafe { async_handle_take_box::<AsyncNextRequestState>(handle) }) else {
        return;
    };
    release_abandoned_async_next_request(&state);
}

// Router-mode async lifecycle aliases: same state machine as pull, different start only.
#[no_mangle]
/// # Safety
///
/// Same contract as [`sengoo_http_server_next_request_async__poll`].
pub unsafe extern "C" fn sengoo_http_server_next_request_router_async__poll(handle: i64) -> i64 {
    unsafe { sengoo_http_server_next_request_async__poll(handle) }
}

#[no_mangle]
/// # Safety
///
/// Same contract as [`sengoo_http_server_next_request_async__result`].
pub unsafe extern "C" fn sengoo_http_server_next_request_router_async__result(
    handle: i64,
) -> HttpServerNextRequestResult {
    unsafe { sengoo_http_server_next_request_async__result(handle) }
}

#[no_mangle]
/// # Safety
///
/// Same contract as [`sengoo_http_server_next_request_async__cancel`].
pub unsafe extern "C" fn sengoo_http_server_next_request_router_async__cancel(handle: i64) -> bool {
    unsafe { sengoo_http_server_next_request_async__cancel(handle) }
}

#[no_mangle]
/// # Safety
///
/// Same contract as [`sengoo_http_server_next_request_async__drop`].
pub unsafe extern "C" fn sengoo_http_server_next_request_router_async__drop(handle: i64) {
    unsafe { sengoo_http_server_next_request_async__drop(handle) }
}

fn request_text_len(handle: u64, read: fn(&HttpRequestEntry) -> &str) -> i64 {
    reset_last_error();
    net_runtime()
        .http_request_with_entry(handle, |entry| read(entry).len() as i64)
        .unwrap_or_else(fail_i64)
}

fn request_text_copy(
    handle: u64,
    buffer: *mut u8,
    capacity: usize,
    read: fn(&HttpRequestEntry) -> &str,
) -> i64 {
    reset_last_error();
    net_runtime()
        .http_request_with_entry(handle, |entry| {
            super::copy_bytes_to_buffer(read(entry).as_bytes(), buffer, capacity)
        })
        .unwrap_or_else(fail_i64)
}

#[no_mangle]
pub extern "C" fn sengoo_http_request_method_len(handle: u64) -> i64 {
    request_text_len(handle, |entry| &entry.method)
}

#[no_mangle]
pub extern "C" fn sengoo_http_request_method_copy(
    handle: u64,
    buffer: *mut u8,
    capacity: usize,
) -> i64 {
    request_text_copy(handle, buffer, capacity, |entry| &entry.method)
}

#[no_mangle]
pub extern "C" fn sengoo_http_request_path_len(handle: u64) -> i64 {
    request_text_len(handle, |entry| &entry.path)
}

#[no_mangle]
pub extern "C" fn sengoo_http_request_path_copy(
    handle: u64,
    buffer: *mut u8,
    capacity: usize,
) -> i64 {
    request_text_copy(handle, buffer, capacity, |entry| &entry.path)
}

#[no_mangle]
pub extern "C" fn sengoo_http_request_query_len(handle: u64) -> i64 {
    request_text_len(handle, |entry| &entry.query)
}

#[no_mangle]
pub extern "C" fn sengoo_http_request_query_copy(
    handle: u64,
    buffer: *mut u8,
    capacity: usize,
) -> i64 {
    request_text_copy(handle, buffer, capacity, |entry| &entry.query)
}

#[no_mangle]
pub extern "C" fn sengoo_http_request_version_len(handle: u64) -> i64 {
    request_text_len(handle, |entry| &entry.version)
}

#[no_mangle]
pub extern "C" fn sengoo_http_request_version_copy(
    handle: u64,
    buffer: *mut u8,
    capacity: usize,
) -> i64 {
    request_text_copy(handle, buffer, capacity, |entry| &entry.version)
}

/// Returns the header value length, `-1` with `last_error == Ok` when the
/// header is absent (distinguishable from an empty value, which returns 0).
#[no_mangle]
pub extern "C" fn sengoo_http_request_header_len(handle: u64, name: *const u8) -> i64 {
    reset_last_error();
    let name = match parse_host(name) {
        Ok(name) => name.to_ascii_lowercase(),
        Err(code) => return fail_i64(code),
    };
    net_runtime()
        .http_request_with_entry(handle, |entry| {
            entry
                .headers
                .get(&name)
                .map(|value| value.len() as i64)
                .unwrap_or(-1)
        })
        .unwrap_or_else(fail_i64)
}

#[no_mangle]
pub extern "C" fn sengoo_http_request_header_copy(
    handle: u64,
    name: *const u8,
    buffer: *mut u8,
    capacity: usize,
) -> i64 {
    reset_last_error();
    let name = match parse_host(name) {
        Ok(name) => name.to_ascii_lowercase(),
        Err(code) => return fail_i64(code),
    };
    net_runtime()
        .http_request_with_entry(handle, |entry| {
            entry
                .headers
                .get(&name)
                .map(|value| super::copy_bytes_to_buffer(value.as_bytes(), buffer, capacity))
                .unwrap_or(-1)
        })
        .unwrap_or_else(fail_i64)
}

#[no_mangle]
pub extern "C" fn sengoo_http_request_body_len(handle: u64) -> i64 {
    reset_last_error();
    net_runtime()
        .http_request_with_entry(handle, |entry| entry.body.len() as i64)
        .unwrap_or_else(fail_i64)
}

#[no_mangle]
pub extern "C" fn sengoo_http_request_body_copy(
    handle: u64,
    buffer: *mut u8,
    capacity: usize,
) -> i64 {
    reset_last_error();
    net_runtime()
        .http_request_with_entry(handle, |entry| {
            super::copy_bytes_to_buffer(&entry.body, buffer, capacity)
        })
        .unwrap_or_else(fail_i64)
}

fn should_keep_alive(
    keep_alive_enabled: bool,
    client_wants_close: bool,
    answered_after: u32,
    version: &str,
) -> bool {
    keep_alive_enabled
        && !client_wants_close
        && answered_after < HTTP_KEEP_ALIVE_MAX_REQUESTS
        && version.to_ascii_uppercase().starts_with("HTTP/1.1")
}

fn write_stream_headers(
    stream: &mut HttpServerConn,
    status: i32,
    content_length: Option<usize>,
    keep_alive: bool,
) -> Result<(), NetErrorCode> {
    let mut head = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: text/plain; charset=utf-8\r\nConnection: {}\r\n",
        status,
        http_reason_phrase(status),
        if keep_alive { "keep-alive" } else { "close" },
    );
    match content_length {
        Some(len) => head.push_str(&format!("Content-Length: {len}\r\n\r\n")),
        None => head.push_str("Transfer-Encoding: chunked\r\n\r\n"),
    }
    stream
        .write_all(head.as_bytes())
        .map_err(|err| classify_io_error(&err))?;
    stream.flush().map_err(|err| classify_io_error(&err))
}

fn write_chunked_body_chunk(stream: &mut HttpServerConn, chunk: &[u8]) -> Result<(), NetErrorCode> {
    let header = format!("{:x}\r\n", chunk.len());
    stream
        .write_all(header.as_bytes())
        .map_err(|err| classify_io_error(&err))?;
    stream
        .write_all(chunk)
        .map_err(|err| classify_io_error(&err))?;
    stream
        .write_all(b"\r\n")
        .map_err(|err| classify_io_error(&err))?;
    stream.flush().map_err(|err| classify_io_error(&err))
}

fn write_chunked_terminator(stream: &mut HttpServerConn) -> Result<(), NetErrorCode> {
    stream
        .write_all(b"0\r\n\r\n")
        .map_err(|err| classify_io_error(&err))?;
    stream.flush().map_err(|err| classify_io_error(&err))
}

fn begin_stream_impl(request_handle: u64, status: i32, content_length: Option<usize>) -> u64 {
    if !(100..=599).contains(&status) {
        return fail_handle(NetErrorCode::InvalidArgument);
    }
    if let Some(len) = content_length {
        if len > usize::try_from(i64::MAX).unwrap_or(usize::MAX) {
            return fail_handle(NetErrorCode::InvalidArgument);
        }
    }

    let entry = match net_runtime().http_request_take(request_handle) {
        Ok(entry) => entry,
        Err(code) => return fail_handle(code),
    };

    let keep_alive_enabled = net_runtime()
        .http_server_keep_alive_enabled(entry.server_handle)
        .unwrap_or(false);
    // Keep-alive decision uses the count after this response completes; headers
    // must match finish() so the client sees a stable Connection value.
    let answered_after = entry.answered_before.saturating_add(1);
    let keep_alive = should_keep_alive(
        keep_alive_enabled,
        entry.client_wants_close,
        answered_after,
        &entry.version,
    );

    let mut stream = entry.stream;
    if let Err(code) = write_stream_headers(&mut stream, status, content_length, keep_alive) {
        return fail_handle(code);
    }

    let mode = match content_length {
        Some(remaining) => HttpStreamBodyMode::Fixed { remaining },
        None => HttpStreamBodyMode::Chunked,
    };
    let stream_entry = HttpResponseStreamEntry {
        server_handle: entry.server_handle,
        stream,
        mode,
        finished: false,
        answered_before: entry.answered_before,
        client_wants_close: entry.client_wants_close,
        version: entry.version,
    };
    net_runtime()
        .http_response_stream_store(stream_entry)
        .unwrap_or_else(fail_handle)
}

fn respond_impl(
    handle: u64,
    status: i32,
    content_type: Option<String>,
    body: *const u8,
    body_len: usize,
) -> i64 {
    if !(100..=599).contains(&status) {
        return fail_bool(NetErrorCode::InvalidArgument);
    }
    let body = match read_c_buffer(body, body_len) {
        Ok(body) => body,
        Err(code) => return fail_bool(code),
    };
    let max_body_bytes =
        match net_runtime().http_request_with_entry(handle, |entry| entry.max_body_bytes) {
            Ok(max) => max,
            Err(code) => return fail_bool(code),
        };
    if body.len() > max_body_bytes {
        return fail_bool(NetErrorCode::InvalidArgument);
    }

    let mut entry = match net_runtime().http_request_take(handle) {
        Ok(entry) => entry,
        Err(code) => return fail_bool(code),
    };
    let mut headers = Vec::new();
    if let Some(content_type) = content_type {
        headers.push(("Content-Type".to_string(), content_type));
    }

    let keep_alive_enabled = net_runtime()
        .http_server_keep_alive_enabled(entry.server_handle)
        .unwrap_or(false);
    let answered_after = entry.answered_before.saturating_add(1);
    let keep_alive = should_keep_alive(
        keep_alive_enabled,
        entry.client_wants_close,
        answered_after,
        &entry.version,
    );

    match write_http_response(
        &mut entry.stream,
        &HttpServerResponse {
            status,
            headers,
            body,
        },
        keep_alive,
    ) {
        Ok(()) => {
            if keep_alive {
                let _ = net_runtime().http_server_put_live_connection(
                    entry.server_handle,
                    LiveHttpConnection {
                        stream: entry.stream,
                        answered_count: answered_after,
                    },
                );
            }
            1
        }
        Err(code) => fail_bool(code),
    }
}

#[no_mangle]
pub extern "C" fn sengoo_http_request_respond(
    handle: u64,
    status: i32,
    body: *const u8,
    body_len: usize,
) -> i64 {
    reset_last_error();
    respond_impl(handle, status, None, body, body_len)
}

#[no_mangle]
pub extern "C" fn sengoo_http_request_respond_with_content_type(
    handle: u64,
    status: i32,
    content_type: *const u8,
    body: *const u8,
    body_len: usize,
) -> i64 {
    reset_last_error();
    let content_type = match parse_host(content_type) {
        Ok(value) if !value.trim().is_empty() => value,
        Ok(_) => return fail_bool(NetErrorCode::InvalidArgument),
        Err(code) => return fail_bool(code),
    };
    respond_impl(handle, status, Some(content_type), body, body_len)
}

/// Closing an unanswered request writes the deterministic 504 fallback first.
#[no_mangle]
pub extern "C" fn sengoo_http_request_close(handle: u64) -> i64 {
    reset_last_error();
    match net_runtime().http_request_take(handle) {
        Ok(mut entry) => {
            let _ = write_http_response(&mut entry.stream, &gateway_timeout_response(), false);
            1
        }
        Err(NetErrorCode::HandleNotFound) => 1,
        Err(code) => fail_bool(code),
    }
}

/// Begin a chunked streamed response. Consumes the request handle (answer-once).
#[no_mangle]
pub extern "C" fn sengoo_http_request_begin_stream(request_handle: u64, status: i32) -> u64 {
    reset_last_error();
    begin_stream_impl(request_handle, status, None)
}

/// Begin a fixed-length streamed response. Consumes the request handle.
#[no_mangle]
pub extern "C" fn sengoo_http_request_begin_stream_with_length(
    request_handle: u64,
    status: i32,
    content_length: u64,
) -> u64 {
    reset_last_error();
    let len = match usize::try_from(content_length) {
        Ok(len) => len,
        Err(_) => return fail_handle(NetErrorCode::InvalidArgument),
    };
    begin_stream_impl(request_handle, status, Some(len))
}

/// Write one body chunk (鈮?65536 bytes). Disconnect 鈫?STATUS_IO; timeout 鈫?STATUS_TIMEOUT.
#[no_mangle]
pub extern "C" fn sengoo_http_response_stream_write(
    stream_handle: u64,
    data: *const u8,
    len: usize,
) -> i64 {
    reset_last_error();
    if len > HTTP_STREAM_MAX_CHUNK {
        return fail_bool(NetErrorCode::InvalidArgument);
    }
    let chunk = match read_c_buffer(data, len) {
        Ok(chunk) => chunk,
        Err(code) => return fail_bool(code),
    };
    match net_runtime().http_response_stream_with_mut(stream_handle, |entry| {
        if entry.finished {
            return Err(NetErrorCode::HandleNotFound);
        }
        match &mut entry.mode {
            HttpStreamBodyMode::Chunked => {
                write_chunked_body_chunk(&mut entry.stream, &chunk)?;
            }
            HttpStreamBodyMode::Fixed { remaining } => {
                if chunk.len() > *remaining {
                    return Err(NetErrorCode::InvalidArgument);
                }
                entry
                    .stream
                    .write_all(&chunk)
                    .map_err(|err| classify_io_error(&err))?;
                entry
                    .stream
                    .flush()
                    .map_err(|err| classify_io_error(&err))?;
                *remaining -= chunk.len();
            }
        }
        Ok(1_i64)
    }) {
        Ok(v) => v,
        Err(code) => fail_bool(code),
    }
}

/// Finish a streamed response. May recycle the connection under keep-alive.
#[no_mangle]
pub extern "C" fn sengoo_http_response_stream_finish(stream_handle: u64) -> i64 {
    reset_last_error();
    let mut entry = match net_runtime().http_response_stream_take(stream_handle) {
        Ok(entry) => entry,
        Err(code) => return fail_bool(code),
    };
    if entry.finished {
        return fail_bool(NetErrorCode::HandleNotFound);
    }
    match &entry.mode {
        HttpStreamBodyMode::Chunked => {
            if let Err(code) = write_chunked_terminator(&mut entry.stream) {
                return fail_bool(code);
            }
        }
        HttpStreamBodyMode::Fixed { remaining } => {
            if *remaining != 0 {
                // Incomplete fixed body: abort connection (no keep-alive).
                entry.finished = true;
                return fail_bool(NetErrorCode::InvalidArgument);
            }
        }
    }
    entry.finished = true;
    let answered_after = entry.answered_before.saturating_add(1);
    let keep_alive_enabled = net_runtime()
        .http_server_keep_alive_enabled(entry.server_handle)
        .unwrap_or(false);
    let keep_alive = should_keep_alive(
        keep_alive_enabled,
        entry.client_wants_close,
        answered_after,
        &entry.version,
    );
    if keep_alive {
        let _ = net_runtime().http_server_put_live_connection(
            entry.server_handle,
            LiveHttpConnection {
                stream: entry.stream,
                answered_count: answered_after,
            },
        );
    }
    1
}

/// Drop/abort a stream handle. Unfinished streams close the TCP connection.
#[no_mangle]
pub extern "C" fn sengoo_http_response_stream_close(stream_handle: u64) -> i64 {
    reset_last_error();
    match net_runtime().http_response_stream_take(stream_handle) {
        Ok(_entry) => {
            // Drop closes the TcpStream 鈫?connection abort for unfinished streams.
            1
        }
        Err(NetErrorCode::HandleNotFound) => 1,
        Err(code) => fail_bool(code),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dynamic_request_stream_reports_timeout_configuration_errors() {
        let _guard = super::super::net_test_lock();
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind test listener");
        let address = listener.local_addr().expect("read test listener address");
        let client = TcpStream::connect(address).expect("connect test client");
        let (server, _) = listener.accept().expect("accept test client");

        let result = process_dynamic_request_stream(
            0,
            HttpServerConn::Plain(server),
            &[],
            &[],
            16 * 1024,
            1024 * 1024,
            Duration::ZERO,
            0,
        );

        assert!(
            result.is_err(),
            "invalid socket timeout configuration must not be reported as NotReady"
        );
        drop(client);
    }

    #[test]
    fn async_next_request_cancel_unregisters_listener_interest() {
        let _guard = super::super::net_test_lock();
        let baseline = crate::async_runtime::http_listener_interest_count();
        let server = sengoo_http_server_bind(std::ptr::null(), 0);
        assert_ne!(server, 0, "test server should bind");

        let future = sengoo_http_server_next_request_async__start(server, 30_000);
        assert_ne!(future, 0, "async next_request should allocate a future");
        assert_eq!(
            crate::async_runtime::http_listener_interest_count(),
            baseline + 1,
            "pending next_request future should register one listener interest"
        );

        assert!(unsafe { sengoo_http_server_next_request_async__cancel(future) });
        assert_eq!(
            crate::async_runtime::http_listener_interest_count(),
            baseline,
            "canceling pending next_request future should unregister listener interest"
        );

        assert_eq!(sengoo_http_server_close(server), 1);
    }

    fn read_http_response_message(stream: &mut impl Read) -> String {
        let mut raw = Vec::new();
        let mut buf = [0u8; 256];
        let header_end = loop {
            let n = stream.read(&mut buf).expect("read response chunk");
            assert!(n > 0, "unexpected EOF while reading response headers");
            raw.extend_from_slice(&buf[..n]);
            if let Some(pos) = raw.windows(4).position(|w| w == b"\r\n\r\n") {
                break pos;
            }
            assert!(raw.len() < 8192, "response headers too large");
        };
        let headers = String::from_utf8_lossy(&raw[..header_end]).into_owned();
        let content_length = headers
            .lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                if name.eq_ignore_ascii_case("content-length") {
                    value.trim().parse::<usize>().ok()
                } else {
                    None
                }
            })
            .unwrap_or(0);
        let mut body = raw[header_end + 4..].to_vec();
        while body.len() < content_length {
            let n = stream.read(&mut buf).expect("read response body");
            assert!(n > 0, "unexpected EOF while reading response body");
            body.extend_from_slice(&buf[..n]);
        }
        body.truncate(content_length);
        format!("{}\r\n\r\n{}", headers, String::from_utf8_lossy(&body))
    }

    #[test]
    fn keep_alive_reuses_connection_for_sequential_requests() {
        use std::io::Write;
        use std::thread;

        let _guard = super::super::net_test_lock();
        let server = sengoo_http_server_bind(std::ptr::null(), 0);
        assert_ne!(server, 0);
        assert_eq!(sengoo_http_server_set_keep_alive(server, 1), 1);
        let port = sengoo_http_server_local_port(server);
        assert!(port > 0);

        let worker = thread::spawn(move || {
            let mut stream =
                TcpStream::connect(("127.0.0.1", port as u16)).expect("client connect");
            stream
                .write_all(b"GET /a HTTP/1.1\r\nHost: localhost\r\n\r\n")
                .expect("write a");
            let first = read_http_response_message(&mut stream);
            assert!(
                first.contains("HTTP/1.1 200") && first.to_ascii_lowercase().contains("keep-alive"),
                "first: {first}"
            );
            stream
                .write_all(b"GET /b HTTP/1.1\r\nHost: localhost\r\n\r\n")
                .expect("write b");
            let second = read_http_response_message(&mut stream);
            assert!(
                second.contains("HTTP/1.1 200")
                    && second.to_ascii_lowercase().contains("keep-alive"),
                "second: {second}"
            );
            stream
                .write_all(b"GET /c HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
                .expect("write c");
            let third = read_http_response_message(&mut stream);
            assert!(
                third.contains("HTTP/1.1 200")
                    && third.to_ascii_lowercase().contains("connection: close"),
                "third: {third}"
            );
        });

        for _ in 0..3 {
            let req = sengoo_http_server_next_request(server, 5_000);
            assert_ne!(req, 0, "next_request should pull");
            assert_eq!(sengoo_http_request_respond(req, 200, b"ok".as_ptr(), 2), 1);
        }
        worker.join().expect("client join");
        assert_eq!(sengoo_http_server_close(server), 1);
    }

    #[test]
    fn response_stream_chunked_completes_and_drop_aborts() {
        use std::io::{Read, Write};
        use std::thread;

        let _guard = super::super::net_test_lock();
        let server = sengoo_http_server_bind(std::ptr::null(), 0);
        assert_ne!(server, 0);
        let port = sengoo_http_server_local_port(server);

        // Completing stream path.
        let worker = thread::spawn(move || {
            let mut stream =
                TcpStream::connect(("127.0.0.1", port as u16)).expect("client connect");
            stream
                .write_all(b"GET /s HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
                .expect("write");
            let mut raw = Vec::new();
            stream.read_to_end(&mut raw).expect("read all");
            let text = String::from_utf8_lossy(&raw);
            assert!(
                text.contains("Transfer-Encoding: chunked")
                    && text.contains("hello")
                    && text.contains("world"),
                "chunked body: {text}"
            );
        });
        let req = sengoo_http_server_next_request(server, 5_000);
        assert_ne!(req, 0);
        let stream = sengoo_http_request_begin_stream(req, 200);
        assert_ne!(stream, 0);
        assert_eq!(
            sengoo_http_response_stream_write(stream, b"hello".as_ptr(), 5),
            1
        );
        assert_eq!(
            sengoo_http_response_stream_write(stream, b"world".as_ptr(), 5),
            1
        );
        assert_eq!(sengoo_http_response_stream_finish(stream), 1);
        worker.join().expect("client join");

        // Drop without finish aborts: oversize chunk rejected; unfinished close ok.
        let worker2 = thread::spawn(move || {
            let mut stream =
                TcpStream::connect(("127.0.0.1", port as u16)).expect("client2 connect");
            stream
                .write_all(b"GET /t HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
                .expect("write");
            let mut raw = Vec::new();
            let _ = stream.read_to_end(&mut raw);
        });
        let req2 = sengoo_http_server_next_request(server, 5_000);
        assert_ne!(req2, 0);
        let stream2 = sengoo_http_request_begin_stream(req2, 200);
        assert_ne!(stream2, 0);
        let big = vec![b'x'; HTTP_STREAM_MAX_CHUNK + 1];
        assert_eq!(
            sengoo_http_response_stream_write(stream2, big.as_ptr(), big.len()),
            0,
            "oversize chunk must fail"
        );
        assert_eq!(sengoo_http_response_stream_close(stream2), 1);
        let _ = worker2.join();
        assert_eq!(sengoo_http_server_close(server), 1);
    }

    #[test]
    fn response_stream_fixed_length_enforces_remaining() {
        use std::io::{Read, Write};
        use std::thread;

        let _guard = super::super::net_test_lock();
        let server = sengoo_http_server_bind(std::ptr::null(), 0);
        assert_ne!(server, 0);
        let port = sengoo_http_server_local_port(server);
        let worker = thread::spawn(move || {
            let mut stream =
                TcpStream::connect(("127.0.0.1", port as u16)).expect("client connect");
            stream
                .write_all(b"GET /f HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
                .expect("write");
            let mut raw = Vec::new();
            stream.read_to_end(&mut raw).expect("read");
            let text = String::from_utf8_lossy(&raw);
            assert!(
                text.contains("Content-Length: 4") && text.ends_with("abcd"),
                "fixed: {text}"
            );
        });
        let req = sengoo_http_server_next_request(server, 5_000);
        assert_ne!(req, 0);
        let stream = sengoo_http_request_begin_stream_with_length(req, 200, 4);
        assert_ne!(stream, 0);
        assert_eq!(
            sengoo_http_response_stream_write(stream, b"ab".as_ptr(), 2),
            1
        );
        assert_eq!(
            sengoo_http_response_stream_write(stream, b"cd".as_ptr(), 2),
            1
        );
        // Extra byte beyond Content-Length.
        assert_eq!(
            sengoo_http_response_stream_write(stream, b"x".as_ptr(), 1),
            0
        );
        // Remaining is still 0 after failed extra write? We rejected before writing,
        // so remaining is 0 and finish should succeed.
        assert_eq!(sengoo_http_response_stream_finish(stream), 1);
        worker.join().expect("join");
        assert_eq!(sengoo_http_server_close(server), 1);
    }

    #[test]
    fn keep_alive_default_remains_connection_close() {
        use std::io::Write;
        use std::thread;

        let _guard = super::super::net_test_lock();
        let server = sengoo_http_server_bind(std::ptr::null(), 0);
        assert_ne!(server, 0);
        // Keep-alive not enabled (default).
        let port = sengoo_http_server_local_port(server);
        let worker = thread::spawn(move || {
            let mut stream =
                TcpStream::connect(("127.0.0.1", port as u16)).expect("client connect");
            stream
                .write_all(b"GET /x HTTP/1.1\r\nHost: localhost\r\n\r\n")
                .expect("write");
            let text = read_http_response_message(&mut stream);
            assert!(
                text.contains("HTTP/1.1 200")
                    && text.to_ascii_lowercase().contains("connection: close"),
                "default close: {text}"
            );
        });
        let req = sengoo_http_server_next_request(server, 5_000);
        assert_ne!(req, 0);
        assert_eq!(sengoo_http_request_respond(req, 200, b"ok".as_ptr(), 2), 1);
        worker.join().expect("client join");
        assert_eq!(sengoo_http_server_close(server), 1);
    }

    #[test]
    fn serve_mode_claim_is_exclusive_between_pull_and_router() {
        let _guard = super::super::net_test_lock();
        let server = sengoo_http_server_bind(std::ptr::null(), 0);
        assert_ne!(server, 0, "test server should bind");

        assert_eq!(
            sengoo_http_server_claim_serve_mode(server, HTTP_SERVER_MODE_ROUTER),
            1,
            "first router claim should succeed"
        );
        assert_eq!(
            sengoo_http_server_claim_serve_mode(server, HTTP_SERVER_MODE_ROUTER),
            1,
            "idempotent router re-claim should succeed"
        );
        assert_eq!(
            sengoo_http_server_claim_serve_mode(server, HTTP_SERVER_MODE_PULL),
            0,
            "pull claim after router should be rejected"
        );
        assert_eq!(
            super::super::sengoo_net_last_error(),
            i64::from(super::super::SENGOO_NET_ERR_INVALID_ARGUMENT),
            "mode conflict maps to invalid argument"
        );

        // Pull next_request after router claim must also fail closed.
        assert_eq!(
            sengoo_http_server_next_request(server, 1),
            0,
            "public pull API must reject after router mode is claimed"
        );

        assert_eq!(sengoo_http_server_close(server), 1);

        let server2 = sengoo_http_server_bind(std::ptr::null(), 0);
        assert_ne!(server2, 0, "second test server should bind");
        assert_eq!(
            sengoo_http_server_claim_serve_mode(server2, HTTP_SERVER_MODE_PULL),
            1,
            "first pull claim should succeed"
        );
        assert_eq!(
            sengoo_http_server_claim_serve_mode(server2, HTTP_SERVER_MODE_ROUTER),
            0,
            "router claim after pull should be rejected"
        );
        assert_eq!(sengoo_http_server_close(server2), 1);
    }

    /// RSA PKCS#8 fixtures: Windows Schannel `Identity::from_pkcs8` uses
    /// `ProviderType::rsa_full()` and rejects ECDSA keys from rcgen defaults.
    fn test_server_pem_bundle() -> (Vec<u8>, Vec<u8>) {
        (
            include_bytes!("testdata/http_server_tls_cert.pem").to_vec(),
            include_bytes!("testdata/http_server_tls_key.pem").to_vec(),
        )
    }

    #[test]
    fn http_server_bind_tls_rejects_empty_and_garbage_pem() {
        let _guard = super::super::net_test_lock();

        let empty = sengoo_http_server_bind_tls(
            std::ptr::null(),
            0,
            std::ptr::null(),
            0,
            std::ptr::null(),
            0,
        );
        assert_eq!(empty, 0, "empty PEM must not bind as TLS");
        let empty_err = super::super::sengoo_net_last_error();
        assert!(
            empty_err == i64::from(super::super::SENGOO_NET_ERR_INVALID_ARGUMENT)
                || empty_err == i64::from(super::super::SENGOO_NET_ERR_TLS_CERT_INVALID),
            "empty PEM maps to invalid arg or cert invalid, got {empty_err}"
        );

        let garbage = b"not-a-pem";
        let bad = sengoo_http_server_bind_tls(
            std::ptr::null(),
            0,
            garbage.as_ptr(),
            garbage.len(),
            garbage.as_ptr(),
            garbage.len(),
        );
        assert_eq!(bad, 0, "garbage PEM must not bind as TLS");
        let bad_err = super::super::sengoo_net_last_error();
        assert!(
            bad_err == i64::from(super::super::SENGOO_NET_ERR_TLS_CERT_INVALID)
                || bad_err == i64::from(super::super::SENGOO_NET_ERR_TLS_HANDSHAKE)
                || bad_err == i64::from(super::super::SENGOO_NET_ERR_TLS_UNAVAILABLE)
                || bad_err == i64::from(super::super::SENGOO_NET_ERR_INVALID_ARGUMENT),
            "garbage PEM maps to STATUS_TLS_*, got {bad_err}"
        );
    }

    #[test]
    fn http_server_tls_handshake_and_pull_response() {
        use std::io::{Read, Write};
        use std::thread;

        trait ReadWrite: Read + Write {}
        impl<T: Read + Write> ReadWrite for T {}

        let _guard = super::super::net_test_lock();
        let (cert_pem, key_pem) = test_server_pem_bundle();

        let server = sengoo_http_server_bind_tls(
            std::ptr::null(),
            0,
            cert_pem.as_ptr(),
            cert_pem.len(),
            key_pem.as_ptr(),
            key_pem.len(),
        );
        assert_ne!(
            server,
            0,
            "TLS server should bind with RSA PKCS#8 test PEM; last_error={}",
            super::super::sengoo_net_last_error()
        );
        let port = sengoo_http_server_local_port(server);
        assert!(port > 0);

        let worker = thread::spawn(move || {
            let tcp = TcpStream::connect(("127.0.0.1", port as u16)).expect("tcp connect");
            tcp.set_read_timeout(Some(Duration::from_secs(5)))
                .expect("read timeout");
            tcp.set_write_timeout(Some(Duration::from_secs(5)))
                .expect("write timeout");

            // Self-signed fixture is not in the system trust store. Prove the
            // *server* handshake with a test client that accepts the presented
            // cert (not a production trust configuration).
            // Windows product stack uses native-tls; POSIX uses rustls — match
            // the client backend so `native_tls` is not required off-Windows.
            #[cfg(windows)]
            let mut tls: Box<dyn ReadWrite> = {
                let connector = native_tls::TlsConnector::builder()
                    .danger_accept_invalid_certs(true)
                    .danger_accept_invalid_hostnames(true)
                    .build()
                    .expect("test connector");
                Box::new(
                    connector
                        .connect("localhost", tcp)
                        .expect("windows native-tls client handshake"),
                )
            };
            #[cfg(not(windows))]
            let mut tls: Box<dyn ReadWrite> = {
                use rustls::client::danger::{
                    HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier,
                };
                use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
                use rustls::{
                    ClientConfig, ClientConnection, DigitallySignedStruct, Error as TlsError,
                    SignatureScheme, StreamOwned,
                };
                use std::sync::Arc;

                #[derive(Debug)]
                struct AcceptAnyCert;
                impl ServerCertVerifier for AcceptAnyCert {
                    fn verify_server_cert(
                        &self,
                        _end_entity: &CertificateDer<'_>,
                        _intermediates: &[CertificateDer<'_>],
                        _server_name: &ServerName<'_>,
                        _ocsp_response: &[u8],
                        _now: UnixTime,
                    ) -> Result<ServerCertVerified, TlsError> {
                        Ok(ServerCertVerified::assertion())
                    }

                    fn verify_tls12_signature(
                        &self,
                        _message: &[u8],
                        _cert: &CertificateDer<'_>,
                        _dss: &DigitallySignedStruct,
                    ) -> Result<HandshakeSignatureValid, TlsError> {
                        Ok(HandshakeSignatureValid::assertion())
                    }

                    fn verify_tls13_signature(
                        &self,
                        _message: &[u8],
                        _cert: &CertificateDer<'_>,
                        _dss: &DigitallySignedStruct,
                    ) -> Result<HandshakeSignatureValid, TlsError> {
                        Ok(HandshakeSignatureValid::assertion())
                    }

                    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
                        rustls::crypto::ring::default_provider()
                            .signature_verification_algorithms
                            .supported_schemes()
                    }
                }

                let _ = rustls::crypto::ring::default_provider().install_default();
                let config = ClientConfig::builder()
                    .dangerous()
                    .with_custom_certificate_verifier(Arc::new(AcceptAnyCert))
                    .with_no_client_auth();
                let name = ServerName::try_from("localhost").expect("server name");
                let conn = ClientConnection::new(Arc::new(config), name).expect("client conn");
                let mut stream = StreamOwned::new(conn, tcp);
                while stream.conn.is_handshaking() {
                    stream
                        .conn
                        .complete_io(&mut stream.sock)
                        .expect("posix rustls handshake");
                }
                Box::new(stream)
            };

            tls.write_all(b"GET /tls HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
                .expect("write request over TLS");
            tls.flush().expect("flush");
            let mut out = Vec::new();
            let mut buf = [0u8; 512];
            let header_end = loop {
                match tls.read(&mut buf) {
                    Ok(0) => panic!("EOF before HTTP headers over TLS"),
                    Ok(n) => out.extend_from_slice(&buf[..n]),
                    Err(err) => panic!("read tls headers: {err}"),
                }
                if let Some(pos) = out.windows(4).position(|w| w == b"\r\n\r\n") {
                    break pos;
                }
                assert!(out.len() < 8192, "headers too large");
            };
            let headers = String::from_utf8_lossy(&out[..header_end]);
            let content_length = headers
                .lines()
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    if name.eq_ignore_ascii_case("content-length") {
                        value.trim().parse::<usize>().ok()
                    } else {
                        None
                    }
                })
                .unwrap_or(0);
            let mut body = out[header_end + 4..].to_vec();
            while body.len() < content_length {
                match tls.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => body.extend_from_slice(&buf[..n]),
                    Err(err) if err.kind() == ErrorKind::WouldBlock => break,
                    Err(err) if err.kind() == ErrorKind::TimedOut => break,
                    Err(err) => panic!("read tls body: {err}"),
                }
            }
            body.truncate(content_length);
            let text = format!("{}\r\n\r\n{}", headers, String::from_utf8_lossy(&body));
            assert!(
                text.contains("HTTP/1.1 200") && text.contains("tls-ok"),
                "TLS HTTP response: {text}"
            );
        });

        let req = sengoo_http_server_next_request(server, 10_000);
        assert_ne!(req, 0, "TLS pull should deliver request after handshake");
        assert_eq!(
            sengoo_http_request_respond(req, 200, b"tls-ok".as_ptr(), 6),
            1
        );
        worker.join().expect("tls client join");
        assert_eq!(sengoo_http_server_close(server), 1);
    }
}
