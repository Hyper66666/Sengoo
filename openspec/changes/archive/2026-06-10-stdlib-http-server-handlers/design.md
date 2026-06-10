## Context

`runtime/src/net/http_server.rs` already provides a synchronous test-grade
server: `bind`, `set_limits` (header/body caps), static `add_route`
(fixed status/body, `:param` path patterns with `{param}` body templating),
`add_middleware_require_header`, `add_ws_echo_route`, and `serve_once`
(accept one connection, read one HTTP/1.1 request with chunked decoding,
answer, `Connection: close`). The Sengoo surface lives in `tools/stdlib/net.sg`
(extern list) and `tools/stdlib/http.sg` (wrappers), with the C-only fallback
bundle (`tools/stdlib/runtime_breadth.c`) returning `STATUS_UNSUPPORTED`.

What is missing is any way to run user Sengoo code per request. Constraints:

- No reliance on cross-FFI user callbacks: the dynamic native ABI is i64-based
  and callback dispatch into Sengoo closures from a runtime-owned accept loop
  is untested territory (reentrancy, panics across the boundary).
- Stdlib conventions are pinned: handle + `Result<T, i64>` status returns,
  `&str` helpers over raw pointers, bounded resources, shell-free, no implicit
  TLS, additive naming.
- The cooperative async runtime exists but async serving is explicitly a later
  reactor-backed lane; this change must not block or pre-empt it.

## Goals / Non-Goals

**Goals:**

- A Sengoo program can serve dynamic HTTP: read request facts, compute, and
  send a response, in a loop, without recompiling routes into the binary.
- Keep the existing static-route/middleware/ws-echo subset source-compatible
  and behaviorally unchanged when no dynamic pull is used.
- Deterministic resource behavior: every accepted request is answered exactly
  once (user response or fallback), bounded queues, stable `std::status`
  mapping for timeout/limit/protocol failures.
- LSP signatures, realworld fixture, support-matrix row, and docs land with the
  API (mainstream-usable-loop conventions).

**Non-Goals:**

- No TLS/HTTPS server (client-only TLS remains the `stdlib-https-tls` scope).
- No streaming request/response bodies; bounded byte vectors only.
- No keep-alive/pipelining; the `Connection: close` per-request model stays.
- No async/await serve integration, no multi-threaded accept pool (a later
  reactor-backed change owns that).
- No user-defined middleware kinds beyond the existing require-header one.

## Decisions

### D1: Pull-based request loop, not handler callbacks

`server_next_request(handle, timeout_ms) -> Result<HttpServerRequestHandle, i64>`
blocks (bounded by timeout) for the next dynamic request; user code inspects it
and must answer via `server_respond*`. Alternatives considered:

- *Handler callback registration* (Go-style): requires calling Sengoo function
  pointers from the runtime accept path across the C ABI; reentrancy, panic
  unwinding, and arity limits (callback bridge caps at 6 i64 args) make this
  high-risk for v1. Deferred, not rejected.
- *Async-await serving*: depends on reactor-backed accept readiness; explicitly
  a later lane. The pull API is forward-compatible: a future
  `await server.next_request()` can reuse the same queue semantics.

Pull keeps all user code on the user's own call stack (same model as
`tiny_http`), needs zero new compiler features, and matches the existing
synchronous `serve_once` execution model.

### D2: Request handle owns the connection

The request handle owns the parsed request plus the `TcpStream`. `respond`
writes the response, flushes, closes the stream, and frees the handle
(generation-checked like other stdlib handles). Closing an unanswered handle -
or closing the server with queued/unanswered requests - writes a deterministic
`504` fallback body first. This guarantees "answered exactly once" without a
background reaper.

### D3: Static routes and middleware stay in front

Inside `server_next_request`, middleware rejection, static routes, and ws-echo
routes are evaluated first and answered inline exactly as `serve_once` does
today; only unmatched requests surface as dynamic request handles. This keeps
existing fixtures' behavior unchanged and gives dynamic serving the same
guardrails (header caps before parse, require-header before user code).

### D4: Bounded introspection and response API (i64 ABI + `&str` wrappers)

New externs (native Rust; C bundle returns `STATUS_UNSUPPORTED`):

- `sengoo_http_server_next_request(handle, timeout_ms) -> req_handle`
- `sengoo_http_request_method/path/query/version` length+copy pairs (Buffer
  protocol identical to existing `sengoo_http_body_len`/`body_copy`)
- `sengoo_http_request_header_value(req, name) -> len/copy` (single header
  lookup by lowercase name; no header iteration in v1)
- `sengoo_http_request_body_len/copy`
- `sengoo_http_request_respond(req, status, body, body_len) -> status`
- `sengoo_http_request_respond_with_content_type(req, status, content_type, body, body_len)`
- `sengoo_http_request_close(req)` (504 fallback if unanswered)

`tools/stdlib/http.sg` wraps these as `HttpServer.next_request(timeout_ms)`,
`HttpServerRequest.method_string()/path_string()/query_string()/header_string(name)/
body_copy(buffer)/respond(status, body)/respond_with_content_type(...)/close()`,
following the owned-`String` return conventions from `stdlib-production-surface`
for the `_string` getters.

### D5: Limits and status mapping

- Pending-dynamic-request cap: fixed at 64 queued unanswered handles per
  server; exceeding it answers new requests `503` inline and returns them to
  the accept loop (request never lost silently).
- `timeout_ms` on `next_request` maps expiry to `STATUS_TIMEOUT`; read/write
  socket failures map through the existing `classify_io_error` net-error to
  `std::status` path; header/body cap violations keep `HttpProtocolError`
  behavior.
- Response body size is capped by the existing `max_body_bytes` limit
  (set via `set_limits`, same default), keeping memory bounded on both sides.

### D6: Verification strategy

Runtime-level Rust tests drive a client thread against a bound server (same
pattern as existing `net::tls`/server tests); `sgc` stdlib e2e compiles a
Sengoo fixture that serves one request to itself over localhost; realworld
fixture `examples/realworld/http-echo-service` runs the locked `sgpm` loop.

## Risks / Trade-offs

- [Single-threaded serial serving limits throughput] -> documented as v1 scope;
  the accept loop already sets the stage for the reactor lane; matrix row says
  "Supported subset" with explicit serial semantics.
- [Slow/stalled client blocks `next_request` during body read] -> per-stream
  read timeout derived from the `timeout_ms` budget; cap violations and
  timeouts answer `400`/`408`-class fallbacks inline and keep the loop alive.
- [Handle leaks if user never responds] -> generation-checked handles,
  `close`-writes-504 fallback, server `close` drains every queued handle, and
  native tests assert the table is empty after drain.
- [Behavior drift between native and C bundle] -> C bundle stubs return
  `STATUS_UNSUPPORTED` for all new symbols (same policy as the existing server
  subset); `sgc` hardening test asserts the stub mapping.
- [Query/path encoding ambiguity] -> v1 exposes raw request-target split at the
  first `?`; no percent-decoding (documented; decoding helpers can land in a
  stdlib followup without ABI changes).

## Migration Plan

Purely additive: no existing extern changes shape, `serve_once` behavior is
untouched, existing examples keep passing. Rollback is removing the new
externs/wrappers; no lockfile, manifest, or cache format is involved.

## Open Questions

- None blocking implementation. (Future lanes: callback-style handlers once the
  callback bridge hardening lane matures; async serve once reactor accept
  readiness lands; keep-alive support if realworld demand appears.)
