## Why

`async-http-serving` archived with reactor-backed dynamic serving proven
end to end, but it explicitly deferred TLS server, keep-alive, streaming
bodies, and callback handlers. What ships today is a serial,
`Connection: close`, plaintext request/answer loop that applications drive
by hand-pulling requests. That demonstrates the runtime; it does not host a
real service: every request pays a fresh TCP handshake, large responses
must materialize in memory, routing is application boilerplate, and there
is no transport security story for serving. This is Pillar C of the
`mainstream-adoption-gap-closure` umbrella and the last gap between "can
answer an HTTP request" and "can host an internal production service".

## What Changes

Feature order is pinned (umbrella decision D4): each lands with tests
before the next starts.

1. **Handler-callback routing**: applications register per-route handlers;
   the server dispatches matched requests to handlers and answers unmatched
   routes with the documented status response, replacing hand-written pull
   loops for the common case (the pull API remains).
2. **Opt-in keep-alive**: bounded HTTP/1.1 connection reuse with pinned
   max-requests-per-connection and idle-timeout bounds; the default without
   opting in remains `Connection: close`.
3. **Streaming response bodies**: handlers can stream a response in bounded
   chunks until completion or client disconnect, with stable status mapping
   for disconnect and timeout paths.
4. **TLS server subset**: serving over TLS using the existing client
   stacks (Schannel on Windows, rustls on POSIX) with a real-handshake
   test per stack and the same no-fake-TLS rules as the client rows.

## Capabilities

### Modified Capabilities

- `stdlib-http-server`: ADDED requirements for handler routing, bounded
  keep-alive, streaming response bodies, and the TLS server subset. All
  existing requirements (dynamic pull, static precedence, answer-exactly-
  once, resource bounds, close drain, C-only fallback, reactor cleanup)
  remain unchanged; new behavior is additive and `Connection: close`
  remains the default.

## Impact

- `runtime/src/net/http_server.rs` and `runtime/src/net/tls.rs` (handler
  dispatch, connection reuse, chunked writes, TLS accept path),
  `tools/stdlib/` http wrappers, `examples/realworld/http-echo-service`
  (or a successor fixture) for handlers/keep-alive/streaming,
  `examples/realworld/SUPPORT_MATRIX.md`, `docs/`.
- Parent umbrella: `mainstream-adoption-gap-closure` (Pillar C).
- Sequenced after `async-cancellation-semantics` (Pillar B): connection
  teardown and slow-client handling reuse the loser-cancellation and
  prompt-cancel primitives.
- POSIX TLS evidence interacts with the open `six-pillar-gap-closure`
  reference-host item; the TLS-server claim per stack requires a real
  handshake on at least one host of that stack and records the other as
  platform-specific until proven.

## Non-Goals

- No HTTP/2, websockets, proxying, or compression negotiation.
- No request-body streaming (responses only in this wave).
- No TLS client changes; client rows and their evidence rules are owned
  elsewhere.
- No change to the pull-based dynamic API, its bounds, or the C-only
  fallback contract.
- No default keep-alive: `Connection: close` stays the default until a
  later change proves otherwise.
