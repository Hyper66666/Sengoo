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
