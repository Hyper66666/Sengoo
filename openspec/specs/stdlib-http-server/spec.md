# stdlib-http-server Specification

## Purpose
Document the supported pull-based HTTP/1.1 server subset exposed through
`std::net`, including request handles, exactly-once response semantics, resource
bounds, fallback behavior, toolchain coverage, and the realworld echo-service
fixture.
## Requirements
### Requirement: Dynamic requests SHALL be pullable from a bound HTTP server

`std::net` SHALL provide `HttpServer.next_request(timeout_ms)` returning a
generation-checked `HttpServerRequest` for the next inbound HTTP/1.1 request
that is not consumed by middleware, static routes, or ws-echo routes. The call
SHALL block at most `timeout_ms` milliseconds and SHALL map expiry to
`STATUS_TIMEOUT` without tearing down the server. The native C ABI backing this
surface is `sengoo_http_server_next_request(server_handle, timeout_ms)`.

#### Scenario: A dynamic request is pulled

- **WHEN** a client sends `GET /compute` to a bound server with no matching static route
- **THEN** `HttpServer.next_request(timeout_ms)` returns a request handle for that request
- **AND** the server handle remains valid for further pulls

#### Scenario: No request arrives before the timeout

- **WHEN** `HttpServer.next_request(50)` is called and no client connects
- **THEN** the call returns an error mapping to `STATUS_TIMEOUT`
- **AND** a later pull on the same server can still succeed

### Requirement: Existing static handling SHALL answer before the dynamic queue

Middleware rejections, static routes, and ws-echo routes SHALL be evaluated and
answered inside the accept path exactly as `serve_once` does today; only
unmatched requests SHALL surface as dynamic request handles. Existing
static-route programs SHALL keep their observable behavior unchanged.

#### Scenario: Static route still answers inline

- **WHEN** a client requests a path registered via `sengoo_http_server_add_route`
- **THEN** the registered fixed response is written without surfacing a request handle

#### Scenario: Require-header middleware still rejects first

- **WHEN** a request is missing a required header configured via the existing middleware
- **THEN** the configured rejection response is written
- **AND** `HttpServer.next_request(timeout_ms)` never observes that request

### Requirement: Request facts SHALL be readable from the request handle

The request handle SHALL expose method, path (request-target before the first
`?`), raw query string (after the first `?`, empty when absent), HTTP version,
single header lookup by case-insensitive name, and the bounded request body.
Text getters SHALL follow the established length+copy Buffer protocol and the
owned-`String` `_string` helper conventions. Bodies SHALL remain bounded by the
server's `max_body_bytes` limit.

#### Scenario: Method, path, and query are read as owned strings

- **WHEN** a request handle for `POST /items?limit=5 HTTP/1.1` is pulled
- **THEN** `method_string()` returns `POST`, `path_string()` returns `/items`, and `query_string()` returns `limit=5`

#### Scenario: Header lookup is case-insensitive and absent headers are distinguishable

- **WHEN** the request carries `X-Trace: abc`
- **THEN** looking up `x-trace` returns `abc`
- **AND** looking up a missing header name returns a status distinguishable from an empty value

#### Scenario: Request body is copied within bounds

- **WHEN** the request body fits within `max_body_bytes`
- **THEN** `body_len`/`body_copy` return the exact bytes received after chunked decoding where applicable

### Requirement: Every pulled request SHALL be answered exactly once

The request handle SHALL own the connection. `respond(status, body)` and
`respond_with_content_type(status, content_type, body)` SHALL write an HTTP/1.1
response with `Connection: close` semantics, flush, close the stream, and free
the handle. Closing an unanswered handle SHALL first write a deterministic
`504` fallback response. Responding twice SHALL fail with a handle error and
SHALL NOT write a second response.

#### Scenario: User response reaches the client

- **WHEN** user code calls `respond(200, body)` on a pulled request
- **THEN** the client receives status 200 with that body and a correct `Content-Length`
- **AND** the request handle becomes invalid for further calls

#### Scenario: Closing an unanswered request sends the fallback

- **WHEN** `close()` is called on a pulled request that was never answered
- **THEN** the client receives the deterministic `504` fallback response

#### Scenario: Double respond is rejected

- **WHEN** user code calls `respond` a second time on the same handle
- **THEN** the call returns a handle-invalid status error
- **AND** no additional bytes are written to the connection

### Requirement: Dynamic serving SHALL stay within explicit resource bounds

At most 64 pulled-but-unanswered request handles SHALL exist per server;
further inbound dynamic requests SHALL be answered `503` inline without
surfacing a handle and without being silently dropped. Header and body caps
from `set_limits` SHALL keep applying before user code runs. Response bodies
SHALL be capped by the same `max_body_bytes` limit. Socket read/write failures
SHALL map through the existing net-error to `std::status` path.

#### Scenario: Pending cap answers overflow inline

- **WHEN** 64 pulled requests are unanswered and a new dynamic request arrives
- **THEN** the new request is answered `503` inline
- **AND** previously pulled handles remain valid and answerable

#### Scenario: Oversized response body is rejected

- **WHEN** user code calls `respond` with a body larger than `max_body_bytes`
- **THEN** the call returns a status error and the handle stays answerable with a smaller body

### Requirement: Server close SHALL drain unanswered requests deterministically

Closing the server SHALL answer every queued or pulled-but-unanswered request
with the `504` fallback, release every request handle, and leave the handle
tables empty.

#### Scenario: Close drains the queue

- **WHEN** the server is closed while two pulled requests are unanswered
- **THEN** both clients receive the `504` fallback response
- **AND** native tests can assert the request handle table is empty afterwards

### Requirement: The C-only bundle SHALL report the dynamic API as unsupported

All new dynamic-serving symbols SHALL exist in the C fallback bundle and SHALL
return the stable `STATUS_UNSUPPORTED` mapping, matching the policy of the
existing server subset.

#### Scenario: Fallback bundle stays honest

- **WHEN** a program built against the C-only bundle calls `HttpServer.next_request(timeout_ms)`
- **THEN** the call fails with the `STATUS_UNSUPPORTED` mapping rather than a fake success

### Requirement: Toolchain, example, and matrix coverage SHALL land with the API

`sglsp` SHALL expose completions/signatures for the new `std::net` server
items; a realworld fixture SHALL exercise the locked `sgpm` loop against a
dynamic echo service; `SUPPORT_MATRIX.md` SHALL gain an explicit HTTP server
row stating the supported subset (serial, plaintext, `Connection: close`) and
its proof commands.

#### Scenario: Realworld fixture proves the loop

- **WHEN** the realworld dynamic-service fixture runs `sgpm update/check/test/fmt --check/doc/build`
- **THEN** the fixture passes using real `sgc`/`sgpm` binaries
- **AND** its smoke test serves and answers at least one localhost request

#### Scenario: Support matrix row is added

- **WHEN** the change is archived
- **THEN** `SUPPORT_MATRIX.md` contains an HTTP server row with status, host scope, proof, and stable diagnostics columns filled

### Requirement: HTTP server SHALL expose an awaitable dynamic request future

`std::net::HttpServer` SHALL provide an async request API that can be awaited
without blocking the cooperative async runtime thread while waiting for the
next inbound dynamic HTTP/1.1 request. The public source shape SHALL be
`await server.next_request_async(timeout_ms)` or an equivalent method returning
a concrete `Future<HttpServerNextRequestOutcome>` wrapper.

The async API SHALL reuse the same request handle, request-introspection,
response, route/middleware precedence, limits, pending-cap, fallback, and
status taxonomy as synchronous `HttpServer.next_request(timeout_ms)`.

#### Scenario: Async server answers a dynamic request

- **WHEN** an async Sengoo program binds a plaintext HTTP server and awaits
  `server.next_request_async(5000)`
- **AND** a localhost client sends `GET /compute`
- **THEN** the await completes with `HttpServerNextRequestOutcome.is_ok == true`
- **AND** `HttpServerNextRequestOutcome.value` is a `HttpServerRequest`
- **AND** user code can inspect the request and call `respond(200, body)`
- **AND** the client receives the response without a synchronous accept call
  blocking unrelated cooperative async work

#### Scenario: Static routes and middleware still answer before async dynamic pull

- **WHEN** the server has a matching static route or require-header middleware
  rejection
- **THEN** the request is answered inline exactly as it is for synchronous
  `next_request`
- **AND** the async future does not surface a request handle for that request

#### Scenario: Async timeout preserves the server

- **WHEN** `await server.next_request_async(50)` is polled and no client arrives
  before the deadline
- **THEN** the await completes with `Result.err(STATUS_TIMEOUT)`
- **AND** a later synchronous or async request pull on the same server can still
  succeed

### Requirement: Async HTTP request future SHALL clean up reactor interest

The async request future SHALL unregister any reactor interest and release any
owned native resources when it is dropped before completion. Dropping or timing
out the future SHALL NOT close the server handle. If a connection has been
accepted but no `HttpServerRequest` handle has been returned, the runtime SHALL
close or answer that connection deterministically and SHALL NOT leak a partially
constructed request.

This cleanup requirement is scoped to the HTTP request future and SHALL NOT be
documented as general task cancellation or select-loser cancellation support.

#### Scenario: Dropping a pending async request future leaves the server usable

- **WHEN** user code creates `server.next_request_async(5000)` and drops it
  before any client connects
- **THEN** the future unregisters its readiness interest
- **AND** the server remains open
- **AND** a later `await server.next_request_async(5000)` can receive and answer
  a client request

#### Scenario: Accepted but unpublished request is not leaked

- **WHEN** a client connection is accepted by the async future but the request
  cannot be fully parsed before timeout or drop cleanup
- **THEN** no request handle is exposed to user code
- **AND** native handle tables and reactor interest tables remain bounded
- **AND** the client observes a deterministic close or HTTP fallback response

### Requirement: Async HTTP serving SHALL remain host-scoped and fallback-safe

Native hosts that implement async HTTP serving SHALL prove listener readiness,
timeout, drop cleanup, and real `sgc`/`sgpm` package execution. Unsupported
hosts and C-only fallback bundles SHALL expose the same symbols but return
`STATUS_UNSUPPORTED`.

#### Scenario: Fallback bundle reports unsupported

- **WHEN** a program built against the C-only fallback bundle awaits
  `server.next_request_async(timeout_ms)`
- **THEN** the future completes with `STATUS_UNSUPPORTED`
- **AND** no unresolved native symbol is produced

#### Scenario: Support matrix distinguishes sync and async server subsets

- **WHEN** async HTTP serving is documented as supported
- **THEN** `examples/realworld/SUPPORT_MATRIX.md` cites native and realworld
  proof commands for the claimed hosts
- **AND** TLS server, keep-alive, streaming bodies, callback handlers, and broad
  cancellation semantics remain Deferred rows or notes

### Requirement: Requests SHALL be routable via a Sengoo-side HttpRouter

The stdlib SHALL expose `HttpRouter` registration of exact method+path
handlers and a single default handler, plus `serve_http` that dispatches by
pulling requests through the existing async pull API. Handler functions SHALL
have the signature `fn(&mut HttpServerRequest) -> Result<bool, i64>`. Method
and path matching SHALL be exact byte equality with no pattern matching,
normalization, or automatic decoding. The implementation SHALL NOT introduce
a reverse-call ABI in which the Rust runtime invokes Sengoo function
pointers. A listener SHALL use either the pull API or router mode, not both.

#### Scenario: A matched request is answered by its handler

- **WHEN** a request arrives whose method and path equal a registered
  route's method and path bytes
- **THEN** `serve_http` invokes that handler with `&mut HttpServerRequest`
- **AND** the handler answers through answer-exactly-once APIs
- **AND** the application does not hand-pull the request outside the router

#### Scenario: An unmatched route gets the documented response

- **WHEN** a request matches no registration and no default handler exists
- **THEN** the server answers with HTTP 404 automatically

#### Scenario: Handler failure maps to a server error response

- **WHEN** a handler returns `Err(...)`, returns `Ok(false)`, or returns
  without answering
- **THEN** the server answers with HTTP 500 if the request was not already
  answered
- **AND** the connection follows the active connection policy

#### Scenario: Mixing pull and router modes is rejected

- **WHEN** an application attempts to use the pull API on a listener already
  in router mode (or attaches a router after pull has been used)
- **THEN** the call fails with a stable status and serving invariants remain
  intact

### Requirement: Keep-alive SHALL be opt-in and bounded

The server SHALL support opt-in HTTP/1.1 connection reuse bounded by
`max_requests_per_connection = 100` and `idle_timeout_ms = 30000`. Without opting
in, the default behavior SHALL remain `Connection: close`.

#### Scenario: Sequential requests reuse one connection

- **WHEN** keep-alive is enabled and a client sends sequential requests
  within the pinned bounds on one connection
- **THEN** the server answers all of them on that connection without
  closing between requests

#### Scenario: A bound is exceeded

- **WHEN** the per-connection request cap is reached or the idle timeout
  elapses
- **THEN** the server closes the connection after any in-flight response
  with the documented behavior

#### Scenario: The default remains close

- **WHEN** keep-alive has not been enabled on a server
- **THEN** every response carries the existing `Connection: close` behavior
- **AND** previously specified pending caps, drain-on-close, and timeout
  semantics hold unchanged under keep-alive as well

### Requirement: Response bodies SHALL be streamable in bounded chunks

Handlers SHALL be able to stream a response body in chunks no larger than
65536 bytes until
completion or client disconnect, with stable status mapping for disconnect
and timeout paths and without violating answer-exactly-once accounting.

#### Scenario: A streamed response completes

- **WHEN** a handler begins a streamed response and writes chunks within
  the pinned chunk bound until finishing
- **THEN** the client receives the full body using the documented transfer
  encoding
- **AND** a cleanly finished stream may keep the connection alive within
  keep-alive bounds

#### Scenario: The client disconnects mid-stream

- **WHEN** the client disconnects while a handler is streaming
- **THEN** the handler's subsequent write or finish call returns a stable
  status
- **AND** the connection closes without double-answer accounting violations

### Requirement: The server SHALL support a TLS subset on existing platform stacks

The server SHALL accept TLS connections using the same platform TLS stacks
as the client rows, claim support per stack only with a real handshake
proof, and never report plaintext fallback or disabled verification as TLS
success.

#### Scenario: A real TLS handshake completes

- **WHEN** the TLS server subset is configured with a certificate and key
  signed by the test certificate authority on a supported host
- **THEN** a client completes a real TLS handshake through the platform
  stack and receives the HTTP response over TLS

#### Scenario: TLS failures map to stable statuses

- **WHEN** the handshake fails (bad certificate, unsupported configuration,
  or stack unavailability)
- **THEN** the failure maps to the existing `STATUS_TLS_*` taxonomy
- **AND** no plaintext fallback is reported as TLS success

#### Scenario: Unproven stacks are recorded, not claimed

- **WHEN** a platform stack lacks a real-handshake proof on an available
  host
- **THEN** the support matrix records that stack as platform-specific with
  the blocking reason instead of claiming it
