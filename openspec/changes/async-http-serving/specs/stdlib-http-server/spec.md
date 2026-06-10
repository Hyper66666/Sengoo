## ADDED Requirements
### Requirement: HTTP server SHALL expose an awaitable dynamic request future

`std::net::HttpServer` SHALL provide an async request API that can be awaited
without blocking the cooperative async runtime thread while waiting for the
next inbound dynamic HTTP/1.1 request. The public source shape SHALL be
`await server.next_request_async(timeout_ms)` or an equivalent method returning
a concrete `Future<Result<HttpServerRequest, i64>>` wrapper.

The async API SHALL reuse the same request handle, request-introspection,
response, route/middleware precedence, limits, pending-cap, fallback, and
status taxonomy as synchronous `HttpServer.next_request(timeout_ms)`.

#### Scenario: Async server answers a dynamic request

- **WHEN** an async Sengoo program binds a plaintext HTTP server and awaits
  `server.next_request_async(5000)`
- **AND** a localhost client sends `GET /compute`
- **THEN** the await completes with `Result.ok(HttpServerRequest)`
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
