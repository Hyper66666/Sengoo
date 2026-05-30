# runtime_net ABI Note

Drivers: `runtime/src/reflect/runtime_net_bench.rs` and public extern surface in `runtime/src/net.rs`

Net error helpers:

- `sengoo_net_last_error() -> i32`
- `sengoo_net_clear_error()`
- `sengoo_net_error_name_copy(code: i32, buffer: *mut u8, capacity: usize) -> i64`

TCP:

- `sengoo_tcp_connect(host: *const u8, port: u16, timeout_ms: u32) -> u64`
- `sengoo_tcp_send(handle: u64, data: *const u8, len: usize) -> i64`
- `sengoo_tcp_recv(handle: u64, buffer: *mut u8, capacity: usize, timeout_ms: u32) -> i64`
- `sengoo_tcp_close(handle: u64) -> i64`

UDP:

- `sengoo_udp_bind(host: *const u8, port: u16) -> u64`
- `sengoo_udp_connect(handle: u64, host: *const u8, port: u16) -> i64`
- `sengoo_udp_send(handle: u64, data: *const u8, len: usize) -> i64`
- `sengoo_udp_recv(handle: u64, buffer: *mut u8, capacity: usize, timeout_ms: u32) -> i64`
- `sengoo_udp_close(handle: u64) -> i64`

HTTP / WebSocket:

- `sengoo_http_get(url: *const u8, timeout_ms: u32) -> u64`
- `sengoo_http_post(url: *const u8, body: *const u8, body_len: usize, timeout_ms: u32) -> u64`
- `sengoo_http_status(handle: u64) -> i64`
- `sengoo_http_body_len(handle: u64) -> i64`
- `sengoo_http_body_copy(handle: u64, buffer: *mut u8, capacity: usize) -> i64`
- `sengoo_http_close(handle: u64) -> i64`
- `sengoo_ws_connect(url: *const u8, timeout_ms: u32) -> u64`
- `sengoo_ws_send_text(handle: u64, data: *const u8, len: usize) -> i64`
- `sengoo_ws_recv_text(handle: u64, buffer: *mut u8, capacity: usize, timeout_ms: u32) -> i64`
- `sengoo_ws_close(handle: u64) -> i64`

HTTP server:

- `sengoo_http_server_bind(host: *const u8, port: u16) -> u64`
- `sengoo_http_server_local_port(handle: u64) -> i64`
- `sengoo_http_server_set_limits(handle: u64, max_header_bytes: u32, max_body_bytes: u32) -> i64`
- `sengoo_http_server_add_route(handle: u64, method: *const u8, path_pattern: *const u8, status: i32, body: *const u8, body_len: usize) -> i64`
- `sengoo_http_server_add_middleware_require_header(handle: u64, name: *const u8, expected_value: *const u8, reject_status: i32, reject_body: *const u8, reject_body_len: usize) -> i64`
- `sengoo_http_server_add_ws_echo_route(handle: u64, path_pattern: *const u8) -> i64`
- `sengoo_http_server_serve_once(handle: u64, timeout_ms: u32) -> i64`
- `sengoo_http_server_close(handle: u64) -> i64`

Network bench:

- `sengoo_net_bench_last_error_code() -> i32`
- `sengoo_net_bench_last_error_len() -> i64`
- `sengoo_net_bench_last_error_copy(buffer: *mut u8, capacity: usize) -> i64`
- `sengoo_net_bench_last_error_clear() -> i32`
- `sengoo_net_bench_run(connections: u32, rtt_messages_per_connection: u32, broadcast_rounds: u32, payload_bytes: u32, report_buffer: *mut u8, report_capacity: usize) -> i64`

Ownership:

- TCP/UDP/HTTP/WS functions return opaque handles that must be closed through matching close functions.
- Buffers and string pointers are borrowed.

Sengoo wrapper note:

The Sengoo wrapper exposes `&str` helpers for host/URL/text input, HTTP server
routes, and required-header middleware, plus managed `Buffer` helpers for
receive/body/error/bench output. Raw pointer/capacity variants remain available
as `_raw` functions for explicit handoff.
