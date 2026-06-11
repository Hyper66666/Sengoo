## Context

The HTTP server today: bound listener, static-answer precedence, pull-based
dynamic requests (sync and awaitable async), answer-exactly-once handles,
pending caps (503), close-timeout fallback (504), drain-on-close, reactor
interest cleanup on drop, and a C-only `STATUS_UNSUPPORTED` fallback. The
umbrella froze decision D4: feature order handlers → keep-alive → streaming
→ TLS server; keep-alive opt-in and bounded; TLS server reuses client
stacks; HTTP/1.1 only.

## Decisions

### D-C1 Handler routing model

- Registration: exact-path table plus a single default handler; methods are
  matched per registration (path + method key). No pattern/parameter
  routing in this wave.
- Dispatch runs on the existing async serving loop: a matched request is
  passed to the handler, which produces a response through the existing
  answer-exactly-once handle; unmatched requests get the documented
  not-found status response automatically.
- Handlers are Sengoo functions registered through stdlib wrappers; the
  pull API remains available, and a server uses either pull or handlers per
  listener (mixing on one listener is rejected with a stable status) to
  keep answer-exactly-once auditable.
- Handler errors (status-returning failures) map to the documented 500-
  family response; the connection follows the active connection policy.

### D-C2 Keep-alive bounds

- Opt-in per server config: `max_requests_per_connection = 100` and
  `idle_timeout_ms = 30000`, both clamped to positive values by the
  existing close/timeout machinery.
- HTTP/1.1 semantics: reuse unless the client sends `Connection: close`,
  a bound is exceeded, or an error/streaming-abort occurs; server then
  closes after the in-flight response.
- Default remains `Connection: close` when keep-alive is not enabled; all
  existing tests keep passing unchanged.
- Pending-cap (503) and drain-on-close behavior apply per request exactly
  as today; keep-alive must not bypass resource bounds.

### D-C3 Streaming response bodies

- Handler-side API: begin a streamed response (status + headers), write
  bounded chunks, finish; transfer uses `Content-Length` when known up
  front, otherwise `Transfer-Encoding: chunked`.
- Chunk writes are bounded to `max_stream_chunk_bytes = 65536`; client
  disconnect during streaming maps to a stable status returned to the
  handler's write/finish calls and closes the connection without violating
  answer-exactly-once accounting.
- Streaming and keep-alive compose: a cleanly finished streamed response
  may keep the connection alive within bounds; an aborted stream closes.

### D-C4 TLS server subset

- Accept-side TLS on the existing stacks: Schannel (Windows) and rustls
  (POSIX); server certificate + key configuration through stdlib config
  mirroring the client trust-configuration patterns.
- Claim discipline: per stack, a real handshake with the test CA must pass
  on at least one host of that stack; the other host family is recorded
  platform-specific until proven (consistent with the open POSIX
  reference-host item owned by `six-pillar-gap-closure`).
- No `verify=false`-style success, no plaintext fallback counted as TLS
  success; failures map to the existing `STATUS_TLS_*` taxonomy.
- TLS composes with handlers/keep-alive/streaming over the same serving
  loop.

### D-C5 Teardown reuse from Pillar B

- Slow-client and abandoned-connection teardown reuse the cancellation
  primitives from `async-cancellation-semantics` (prompt cancel, no
  dangling reactor interest); this child is sequenced after Pillar B and
  records it as a blocker if unarchived.

## Acceptance targets

| Feature | Target |
| --- | --- |
| Handlers | Routed request answered by handler; unmatched route gets documented status; runtime tests + realworld fixture |
| Keep-alive | N sequential requests on one connection within bounds; bound breach closes; default remains close |
| Streaming | Bounded chunks delivered until finish; disconnect maps to stable status; composes with keep-alive |
| TLS server | Real handshake with test CA per stack on at least one host; `STATUS_TLS_*` on failures |

## Risks / Trade-offs

- Keep-alive widens DoS surface: bounds are mandatory and tested (request
  cap, idle timeout, existing pending cap).
- Handler dispatch must not break answer-exactly-once accounting; the
  one-mode-per-listener rule keeps the invariant auditable.
- Chunked encoding edge cases (zero-length chunk, early disconnect) are
  covered by negative tests before keep-alive composition.
- Schannel server-side APIs differ from client-side; if server Schannel
  proves infeasible in-lane, the Windows row is recorded platform-specific
  with rustls-on-POSIX as the proven stack — not silently dropped.

## Migration Plan

Additive. Pull-based servers keep working unchanged; new config and
registration APIs are opt-in.

## Open Questions

- None before implementation. Request cap, idle timeout, and chunk size are
  pinned in D-C2/D-C3.
