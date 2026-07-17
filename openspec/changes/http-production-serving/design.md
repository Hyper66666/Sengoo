## Context

The HTTP server today: bound listener, static-answer precedence, pull-based
dynamic requests (sync and awaitable async), answer-exactly-once handles,
pending caps (503), close-timeout fallback (504), drain-on-close, reactor
interest cleanup on drop, and a C-only `STATUS_UNSUPPORTED` fallback.

Feature order remains handlers → keep-alive → streaming → TLS server. This
revision freezes the **handler routing architecture** so implementation does
not invent an unsafe Rust→Sengoo callback ABI.

## Decisions

### D-C1 Handler routing model (Sengoo-side, pull-based)

Routing is implemented in Sengoo on top of the **existing async pull API**.
There is **no** new reverse-call ABI where Rust invokes Sengoo function
pointers. The runtime continues to own accept/read/answer accounting; the
stdlib router loop pulls requests and dispatches in Sengoo.

#### Registration surface

```sg
// Handler signature (frozen):
//   fn(&mut HttpServerRequest) -> Result<bool, i64>
//
// Ok(true)  = handler answered via answer-exactly-once APIs
// Ok(false) = handler declined to answer (server emits 500)
// Err(code) = handler failure (server emits 500; code may be logged)

struct HttpRouter { /* opaque handle */ }

def http_router_new() -> Result<HttpRouter, i64>;
def http_router_route(
    router: &mut HttpRouter,
    method: &str,
    path: &str,
    handler: fn(&mut HttpServerRequest) -> Result<bool, i64>,
) -> Result<bool, i64>;
def http_router_default(
    router: &mut HttpRouter,
    handler: fn(&mut HttpServerRequest) -> Result<bool, i64>,
) -> Result<bool, i64>;

// Binds the router to a listener and runs the Sengoo serve loop.
// Pull mode and router mode are mutually exclusive per listener.
def serve_http(server: &HttpServer, router: &HttpRouter) -> Result<bool, i64>;
```

#### Matching rules

- Method and path are **exact byte matches** of the registered strings.
- No pattern matching, no path normalization, no automatic percent-decoding,
  no method override headers.
- At most one default handler per router.

#### Response policy

| Condition | Server behavior |
| --- | --- |
| Exact method+path hit | Invoke registered handler |
| No path/method hit, default registered | Invoke default handler |
| No hit and no default | Answer **404** automatically |
| Handler returns `Err` or `Ok(false)` or leaves request unanswered | Answer **500** if not already answered |
| Double-answer | Existing answer-exactly-once status (`STATUS_INVALID_HANDLE` / documented path) |

#### Mode exclusivity

- A listener uses **either** pull (`next_request` / async pull) **or** router
  (`serve_http`), never both.
- Mixing returns a stable status (`STATUS_INVALID_ARGUMENT` or dedicated
  mode-violation status) without breaking answer accounting.

### D-C2 Keep-alive bounds

- Opt-in per server config: `max_requests_per_connection = 100` and
  `idle_timeout_ms = 30000`, both clamped to positive values.
- Connection state is **server-owned**, not tied to a single request handle
  lifecycle, so reuse survives answer/drop of individual requests.
- HTTP/1.1 semantics: reuse unless the client sends `Connection: close`, a
  bound is exceeded, or an error/streaming-abort occurs; server then closes
  after the in-flight response.
- Default remains `Connection: close` when keep-alive is not enabled.
- Pending-cap (503) and drain-on-close apply per request as today.

### D-C3 Streaming response bodies

- Handler-side API:
  - `HttpResponseStream`
  - `begin_stream` / `begin_stream_with_length`
  - `write_buffer` (chunk ≤ **65536** bytes)
  - `finish`
- Transfer uses `Content-Length` when length known up front, otherwise
  `Transfer-Encoding: chunked`.
- Unfinished Drop of `HttpResponseStream` aborts the stream and **closes** the
  connection.
- Client disconnect → `STATUS_IO` on subsequent write/finish; write timeout →
  `STATUS_TIMEOUT`.
- Cleanly finished stream may keep the connection alive within keep-alive
  bounds; aborted stream closes.

### D-C4 TLS server subset

- `http_server_bind_tls` accepts:
  - PEM certificate chain in a managed `Buffer`
  - PKCS#8 PEM private key in a managed `Buffer`
- Stacks: Windows **native-tls/Schannel**, POSIX **rustls** — **no new
  dependencies**, no plaintext fallback counted as TLS success.
- Failures map to existing `STATUS_TLS_*` (cert invalid, handshake,
  unavailable, hostname mismatch where applicable).
- Claim per stack only with real test-CA handshake evidence on at least one
  host of that stack; otherwise Platform-specific.

### D-C5 Teardown reuse from Pillar B

- Slow-client and abandoned-connection teardown reuse cancellation primitives
  from archived `async-cancellation-semantics`.

## Acceptance targets

| Feature | Target |
| --- | --- |
| Handlers | Sengoo `HttpRouter` + `serve_http`; dual-route realworld; 404/500/mode tests |
| Keep-alive | N sequential requests on one connection within 100/30s bounds; default close |
| Streaming | Bounded chunks; disconnect=`STATUS_IO`; timeout=`STATUS_TIMEOUT`; Drop aborts |
| TLS server | Real handshake with test CA per claimed stack; no plaintext fallback |

## Risks / Trade-offs

- Sengoo-side dispatch keeps the pull loop simple and avoids reverse FFI, at
  the cost of one cooperative poll loop per server (acceptable for v0.2).
- Exact-byte routing is deliberately minimal; frameworks can layer later.
- Keep-alive widens DoS surface: bounds mandatory and tested.
- Schannel server-side may differ from client; unproven stacks stay
  Platform-specific.

## Migration Plan

Additive. Pull-based servers keep working unchanged; router/serve_http,
keep-alive config, streaming, and TLS bind are opt-in.

## Open Questions

- None for handlers. Bounds pinned in D-C2/D-C3.
