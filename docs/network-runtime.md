# Sengoo Runtime Network Baseline

This document defines the current runtime network baseline APIs.

## Scope

- TCP socket connect/send/recv/close
- UDP bind/connect/send/recv/close
- HTTP over TCP (HTTP/1.1, `http://` only)
- WebSocket over TCP (`ws://` only)
- HTTP server runtime (HTTP/1.1 parse + routing + middleware + WS upgrade)
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

- `u64 sengoo_http_server_bind(const u8* host, u16 port)`
- `i64 sengoo_http_server_local_port(u64 server_handle)`
- `i64 sengoo_http_server_set_limits(u64 server_handle, u32 max_header_bytes, u32 max_body_bytes)`
- `i64 sengoo_http_server_add_route(u64 server_handle, const u8* method, const u8* path_pattern, i32 status, const u8* body, usize body_len)`
- `i64 sengoo_http_server_add_middleware_require_header(u64 server_handle, const u8* name, const u8* expected_value, i32 reject_status, const u8* reject_body, usize reject_body_len)`
- `i64 sengoo_http_server_add_ws_echo_route(u64 server_handle, const u8* path_pattern)`
- `i64 sengoo_http_server_serve_once(u64 server_handle, u32 timeout_ms)`
- `i64 sengoo_http_server_close(u64 server_handle)`

### Error Mapping

- `i32 sengoo_net_last_error()`
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

## Current Constraints

- HTTPS (`https://`) and secure WebSocket (`wss://`) are not part of this baseline.
- HTTP keeps baseline behavior (status + body retrieval).
- WebSocket baseline supports text frames for smoke/e2e paths.
- HTTP server middleware/handler model is MVP-level (no async middleware chain yet).

## Verification

- Runtime tests: `cargo test -q -p sengoo-runtime net::tests -- --nocapture`
- Integrated smoke: `bench/scripts/e2e-smoke.sh` and `bench/scripts/e2e-smoke.ps1`
