# Sengoo Runtime Network Baseline

This document defines the current runtime network baseline APIs.

## Scope

- TCP socket connect/send/recv/close
- UDP bind/connect/send/recv/close
- HTTP over TCP (HTTP/1.1, `http://` only)
- WebSocket over TCP (`ws://` only)
- HTTP server runtime (HTTP/1.1 parse + routing + middleware + WS upgrade)
- HTTP server dynamic request serving (synchronous pull or reactor-backed async await + respond)
- Protocol-level error code mapping for network FFI calls

## C ABI API Surface

### TCP

- `u64 sengoo_tcp_connect(const u8* host, u16 port, u32 timeout_ms)`
- `i64 sengoo_tcp_send(u64 handle, const u8* data, usize len)`
- `i64 sengoo_tcp_recv(u64 handle, u8* buffer, usize capacity, u32 timeout_ms)`
- `i64 sengoo_tcp_close(u64 handle)`

### UDP

- `u64 sengoo_udp_bind(const u8* host, u16 port)`
- `i64 sengoo_udp_connect(u64 handle, const u8* host, u16 port)`
- `i64 sengoo_udp_send(u64 handle, const u8* data, usize len)`
- `i64 sengoo_udp_recv(u64 handle, u8* buffer, usize capacity, u32 timeout_ms)`
- `i64 sengoo_udp_close(u64 handle)`

### HTTP (`http://` only)

- `u64 sengoo_http_get(const u8* url, u32 timeout_ms)`
- `u64 sengoo_http_post(const u8* url, const u8* body, usize len, u32 timeout_ms)`
- `i64 sengoo_http_status(u64 response_handle)`
- `i64 sengoo_http_body_len(u64 response_handle)`
- `i64 sengoo_http_body_copy(u64 response_handle, u8* buffer, usize capacity)`
- `i64 sengoo_http_close(u64 response_handle)`

### WebSocket (`ws://` only)

- `u64 sengoo_ws_connect(const u8* url, u32 timeout_ms)`
- `i64 sengoo_ws_send_text(u64 handle, const u8* data, usize len)`
- `i64 sengoo_ws_recv_text(u64 handle, u8* buffer, usize capacity, u32 timeout_ms)`
- `i64 sengoo_ws_close(u64 handle)`

### HTTP Server

- `u64 sengoo_http_server_bind(const u8* host, u16 port)` — plaintext HTTP
- `u64 sengoo_http_server_bind_tls(const u8* host, u16 port, const u8* cert_pem, usize cert_len, const u8* key_pem, usize key_len)` — HTTPS with PEM certificate chain + PKCS#8 PEM private key (Windows: native-tls/Schannel; POSIX: rustls). Empty/invalid PEM maps to `STATUS_TLS_*` / `STATUS_INVALID_ARGUMENT`; never silent plaintext success.
- `i64 sengoo_http_server_local_port(u64 server_handle)`
- `i64 sengoo_http_server_set_limits(u64 server_handle, u32 max_header_bytes, u32 max_body_bytes)`
- `i64 sengoo_http_server_set_keep_alive(u64 server_handle, i64 enabled)` — opt-in HTTP/1.1 keep-alive (max 100 requests / 30s idle; default `Connection: close`)
- `i64 sengoo_http_server_add_route(u64 server_handle, const u8* method, const u8* path_pattern, i32 status, const u8* body, usize body_len)`
- `i64 sengoo_http_server_add_middleware_require_header(u64 server_handle, const u8* name, const u8* expected_value, i32 reject_status, const u8* reject_body, usize reject_body_len)`
- `i64 sengoo_http_server_add_ws_echo_route(u64 server_handle, const u8* path_pattern)` — plain TCP only (TLS WebSocket not productized)
- `i64 sengoo_http_server_serve_once(u64 server_handle, u32 timeout_ms)`
- `i64 sengoo_http_server_close(u64 server_handle)`

### HTTP Server Dynamic Requests

- `u64 sengoo_http_server_next_request(u64 server_handle, u32 timeout_ms)`
- `i64 sengoo_http_server_next_request_async__start(u64 server_handle, u32 timeout_ms)`
- `i64 sengoo_http_server_next_request_async__poll(i64 future_handle)`
- `HttpServerNextRequestResult sengoo_http_server_next_request_async__result(i64 future_handle)`
- `bool sengoo_http_server_next_request_async__cancel(i64 future_handle)`
- `void sengoo_http_server_next_request_async__drop(i64 future_handle)`
- `i64 sengoo_http_request_method_len(u64 request_handle)` / `i64 sengoo_http_request_method_copy(u64 request_handle, u8* buffer, usize capacity)`
- `i64 sengoo_http_request_path_len(u64 request_handle)` / `i64 sengoo_http_request_path_copy(u64 request_handle, u8* buffer, usize capacity)`
- `i64 sengoo_http_request_query_len(u64 request_handle)` / `i64 sengoo_http_request_query_copy(u64 request_handle, u8* buffer, usize capacity)`
- `i64 sengoo_http_request_version_len(u64 request_handle)` / `i64 sengoo_http_request_version_copy(u64 request_handle, u8* buffer, usize capacity)`
- `i64 sengoo_http_request_header_len(u64 request_handle, const u8* name)` / `i64 sengoo_http_request_header_copy(u64 request_handle, const u8* name, u8* buffer, usize capacity)`
- `i64 sengoo_http_request_body_len(u64 request_handle)` / `i64 sengoo_http_request_body_copy(u64 request_handle, u8* buffer, usize capacity)`
- `i64 sengoo_http_request_respond(u64 request_handle, i32 status, const u8* body, usize body_len)`
- `i64 sengoo_http_request_respond_with_content_type(u64 request_handle, i32 status, const u8* content_type, const u8* body, usize body_len)`
- `i64 sengoo_http_request_begin_stream(u64 request_handle, i32 status)` / `i64 sengoo_http_request_begin_stream_with_length(u64 request_handle, i32 status, i64 content_length)` — bounded response streaming (chunked or fixed; max chunk 65536)
- `i64 sengoo_http_response_stream_write(u64 stream_handle, const u8* data, usize len)` / `i64 sengoo_http_response_stream_finish(u64 stream_handle)` / `i64 sengoo_http_response_stream_close(u64 stream_handle)`
- `i64 sengoo_http_request_close(u64 request_handle)`

Sengoo-side routing uses `HttpRouter` + `serve_http` / `serve_http_once` in `std::net` (exact method+path; pull and router modes are exclusive per listener).

### Error Mapping

- `i64 sengoo_net_last_error()`
- `void sengoo_net_clear_error()`
- `i64 sengoo_net_error_name_copy(i32 code, u8* buffer, usize capacity)`

Selected error codes:

- `0`: `ok`
- `1`: `invalid_argument`
- `2`: `invalid_url`
- `3`: `unsupported_scheme`
- `4`: `resolve_failed`
- `5`: `connect_failed`
- `7`: `timeout`
- `8`: `http_protocol_error`
- `9`: `http_chunk_decode_error`
- `10`: `websocket_handshake_error`
- `11`: `websocket_protocol_error`
- `12`: `handle_not_found`
- `14`: `remote_closed`
- `15`: `tls_cert_invalid`
- `16`: `tls_hostname_mismatch`
- `17`: `tls_handshake`
- `18`: `tls_unavailable`

## Sengoo stdlib wrapper

`import std::net` preloads the network wrapper and its `std::ffi` dependency, so
caller-owned output payloads can use managed `Buffer` handles instead of raw
pointer/capacity pairs:

```sg
import std::net;

def main() -> i64 {
    let out = ffi_buffer_new(256);
    if out.is_err() {
        0
    } else {
        let buffer = out.unwrap_or(Buffer { handle: 0 });
        let copied = net_error_name_copy(net_last_error(), buffer);
        buffer.free();
        copied.unwrap_or(0)
    }
}
```

The same `Buffer` pattern is exposed by `TcpStream.recv`,
`UdpSocket.recv`, `HttpClient.body_copy`, `WsClient.recv_text`,
`net_bench_last_error_copy`, and `net_bench_run`.

`HttpServer` wraps the HTTP server baseline with `&str` helpers for bind,
static routes, required-header middleware, and WS echo routes. Its
`serve_once(timeout_ms)` method returns `Ok(true)` after serving one client,
`Ok(false)` on timeout, and `Err(code)` for runtime failures. Example:
`examples/reflection/net_http_server.sg`.

`HttpServer.next_request(timeout_ms)` pulls the next dynamic request as an
`HttpServerRequest` handle. The request handle exposes
`method_string()/path_string()/query_string()/version_string()` (owned
`String` getters), `header_string(name)` (absent headers map to
`STATUS_NOT_FOUND`), `body_len()/body_copy(buffer)`, exactly-once
`respond(status, body)` / `respond_with_content_type(status, content_type, body)`
/ `respond_raw(status, ptr, len)`, and `close()` (answers unanswered requests
with the deterministic `504` fallback). A runnable package fixture lives at
`examples/realworld/http-echo-service`.

Inside an async function, `await server.next_request_async(timeout_ms)` returns
`HttpServerNextRequestOutcome` with the same request and status taxonomy. The
native `HttpServerNextRequestResult.value` field uses the same one-field
`HttpServerRequest { handle }` wrapper shape as the source-level outcome. The
native future registers listener readiness with the cooperative reactor. A
timeout returns `STATUS_TIMEOUT` without closing the server; dropping or
canceling a pending future unregisters its listener interest. Accepted clients
that do not finish a request within the short cooperative I/O slice receive a
best-effort `400` response or are closed, and no partial request handle is
published. C-only fallback bundles keep the lifecycle symbols linkable and
return `STATUS_UNSUPPORTED`.

## Return Conventions

- Handle-returning APIs: `0` means failure.
- Send/recv/copy APIs: `-1` means failure.
- Close APIs: `1` success, `0` failure/not-found.
- On failure/non-success cases, callers should inspect `sengoo_net_last_error()` for protocol-level reason.
- `sengoo_ws_recv_text`:
  - `> 0`: bytes received
  - `0`: close frame / no data (`remote_closed`)
  - `-1`: error

## HTTP Behavior Notes

- Supports `GET` and `POST`.
- Exposes status code and response body through response handle API.
- Supports `Transfer-Encoding: chunked` decoding.
- Chunk decode/protocol parse failures map to explicit protocol errors.

## WebSocket Behavior Notes

- Handshake validation checks:
  - HTTP status `101`
  - `Upgrade: websocket`
  - `Connection: Upgrade`
  - presence of `Sec-WebSocket-Accept`
- `ping` frames are auto-answered with `pong`.
- `close` frame path is supported in both recv and explicit close API.

## HTTP Server Behavior Notes

- Protocol parsing:
  - parses HTTP/1.1 request line, headers, and body framing (`Content-Length` and chunked requests)
  - malformed requests return `400 Bad Request`
- Routing:
  - method + path pattern dispatch
  - supports parameter segments like `/hello/:name`
  - unmatched route returns `404 Not Found`
- Middleware:
  - deterministic registration order
  - current built-in middleware: required header guard (can short-circuit with custom status/body)
- WebSocket upgrade:
  - route-level upgrade endpoint via `sengoo_http_server_add_ws_echo_route`
  - validates upgrade headers and returns `426 Upgrade Required` on invalid upgrade request
  - WS session supports text echo + ping/pong + close
- Dynamic request serving (pull and async models):
  - `sengoo_http_server_next_request` accepts within the timeout budget; the
    serve loop stays serial (one connection at a time) and plaintext-only
  - middleware rejections, static routes, and ws-echo routes are answered
    inline before a request can surface as a dynamic handle, so existing
    `serve_once`-era fixtures keep their behavior
  - the request target splits at the first `?` into path and query; no
    percent-decoding is applied in v1
  - header lookup is case-insensitive; an absent header is distinguishable
    from an empty value (`-1` + `ok` last-error at the C ABI level)
  - every pulled request is answered exactly once: `respond*` writes, flushes,
    closes (`Connection: close`), and frees the handle; double respond fails
    with `handle_not_found`; closing an unanswered handle writes a `504`
    fallback; closing the server drains all queued handles with the same `504`
  - at most 64 pulled-but-unanswered requests per server; overflow requests
    are answered `503` inline and never surface
  - response bodies above the `set_limits` `max_body_bytes` cap are rejected
    with `invalid_argument` while keeping the handle answerable
  - `timeout_ms` expiry maps to the `timeout` net error (`STATUS_TIMEOUT` in
    `std::status`)
  - async listener readiness uses the runtime reactor; pending timeout,
    cancel, result, and drop paths unregister their interest deterministically
  - synchronous `next_request` remains source-compatible and shares parsing,
    routing, middleware, request-handle, and status behavior with the async path

## Current Constraints

- Secure WebSocket (`wss://`) is not part of this baseline.
- HTTP keeps baseline behavior (status + body retrieval).
- WebSocket baseline supports text frames for smoke/e2e paths.
- HTTP server middleware/handler model is MVP-level (no async middleware chain yet).
- Dynamic serving remains serial per `HttpServer`. Routing is Sengoo-side
  (`HttpRouter` / `serve_http` over the pull API); there is no reverse
  Rust→Sengoo callback ABI. Keep-alive is opt-in and bounded, response
  streaming is bounded per chunk, and the TLS server subset composes with all
  three (`tls_composes_with_router_keep_alive_and_streaming`). Request-body
  streaming and HTTP/2 remain out of scope. This baseline does not claim
  general task-cancellation propagation beyond the pending request future's
  own cancel/drop cleanup.

## Verification

- Runtime tests: `cargo test -q -p sengoo-runtime net::tests -- --nocapture --test-threads=1`
- Native async server integration: `cargo test -p sgc stdlib_http_server_async_awaits_and_answers_localhost_request -- --nocapture --test-threads=1`
- Integrated smoke: `bench/scripts/e2e-smoke.sh` and `bench/scripts/e2e-smoke.ps1`
