# Design: async-http-serving

## Existing Baseline
The archived `stdlib-http-server-handlers` wave added a synchronous, dynamic
HTTP/1.1 server subset:

- `HttpServer.next_request(timeout_ms)` blocks for a bounded time and returns an
  unmatched dynamic request handle.
- Static routes, require-header middleware, and ws-echo routes answer inline
  before the dynamic queue.
- Each pulled request owns a connection and must be answered exactly once.
- Closing an unanswered request or server writes a deterministic `504`
  fallback.
- Header/body/response limits, pending caps, status mappings, LSP surface, and
  a realworld echo-service fixture are already in place.

The async runtime baseline includes reactor wakeups, timers, user-defined
`Future<T>::poll`, same-thread cooperative polling, `timeout`, `timeout_cancel`,
and bounded select support. Remaining async default gaps include broad
cancellation semantics and all-host owned-fd readiness.

## Goals
- Let Sengoo user code await HTTP requests without blocking the cooperative
  runtime thread.
- Reuse the existing HTTP server request handle and response semantics.
- Keep native readiness ownership explicit and drop-safe.
- Preserve synchronous server behavior exactly.
- Make support claims host-scoped and evidence-backed.

## Non-Goals
- No TLS/HTTPS server, keep-alive, pipelining, HTTP/2, streaming bodies, or
  callback-style request handlers.
- No claim that `select` loser cancellation is generally solved; only the async
  request future's own drop/timeout cleanup is in scope.
- No cross-thread or multi-worker serving pool in v1.

## Public API Shape
Preferred source shape:

```sg
impl HttpServer {
    def next_request_async(self, timeout_ms: i64) -> Future<HttpServerNextRequestOutcome>;
}

let request_result = await server.next_request_async(5_000);
```

The native runtime returns an opaque future handle at the ABI boundary, while the
source-level stdlib wrapper exposes a normal `Future<HttpServerNextRequestOutcome>`
surface. The public usage must remain `await server.next_request_async(timeout_ms)`.

`next_request_async` returns the same success and error categories as
`next_request`:

- success: a normal `HttpServerRequest` handle
- timeout: `STATUS_TIMEOUT`
- unsupported host/fallback bundle: `STATUS_UNSUPPORTED`
- invalid server handle: `STATUS_INVALID_HANDLE`
- IO/protocol/limit failures: existing `std::status` mapping

## Runtime Shape
The async path should factor the current synchronous accept/read/route logic
instead of duplicating protocol parsing. The native runtime should expose a
pollable operation roughly shaped as:

1. Validate the server handle and clone/snapshot any immutable server state
   needed for the accept attempt.
2. Register listener-read readiness with the async reactor.
3. Return Pending without blocking if no connection is ready.
4. On wakeup, accept at most one connection and run the same middleware/static
   route/ws-echo/dynamic-request flow used by `next_request`.
5. If a dynamic request reaches user code, store the same request handle type
   and return Ready(Result.ok(handle)).

Slow request-body reads must remain bounded by the timeout budget or an
implementation-defined short per-stream budget documented in
`docs/network-runtime.md`. The future must never park the cooperative runtime
thread indefinitely after listener readiness fires.

## Drop, Timeout, And Cleanup
The future owns any reactor interest it registers. Dropping the future before it
becomes Ready SHALL unregister the interest and SHALL NOT close the server.

Timeout behavior belongs to the async future itself:

- If the deadline expires before an inbound dynamic request is produced, polling
  returns Ready(Result.err(STATUS_TIMEOUT)).
- A timeout while reading an accepted connection may answer a deterministic
  HTTP timeout/fallback response inline if bytes were already accepted, but it
  must not surface a half-built request handle.
- Once Ready returns a request handle, the existing request exactly-once
  response and server-close drain rules apply.

This change does not claim general task cancellation or select loser
cancellation. It only requires that this future's drop path releases reactor
interest and any accepted-but-unpublished connection deterministically.

## Host Support
Native support must be proven at least on the existing CI host set used by
`realworld-e2e`. Unsupported hosts or C-only fallback builds must expose the
same symbols and return `STATUS_UNSUPPORTED`.

The support matrix must distinguish:

- synchronous dynamic HTTP server: already supported subset
- async HTTP serving: supported subset only for proven native hosts
- TLS server / keep-alive / streaming: deferred

## LSP And Diagnostics
`sglsp` must expose the new `HttpServer.next_request_async` and concrete future
surface. Rejected misuse should reuse existing async-context/future diagnostics
where possible:

- awaiting the result of synchronous `next_request` should still be rejected as
  a non-future await
- escaping or constructing `AsyncContext` remains rejected
- unsupported runtime bundle returns stable `STATUS_UNSUPPORTED` rather than
  unresolved symbols

## Verification Strategy
Use TDD at three levels:

1. Runtime unit tests for reactor wakeup, timeout, drop cleanup, invalid handle,
   unsupported fallback mapping, and equivalence with existing route/middleware
   precedence.
2. `sgc` tests that compile and run a small async server which serves a real
   localhost client request through generated native code.
3. A realworld package fixture that runs through the locked package loop and
   appears in `SUPPORT_MATRIX.md` with proof commands.

## Risks
- Readiness after accept may still block on slow headers or bodies. Mitigation:
  enforce timeout budgets/caps and test slow-client timeout behavior.
- Shared server state can drift between sync and async paths. Mitigation:
  factor common accept-and-classify logic behind one internal helper.
- Future drop cleanup may be easy to miss. Mitigation: add native tests that
  drop pending futures repeatedly and then prove a later async accept still
  works without leaked interest or handles.
