## ADDED Requirements

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
