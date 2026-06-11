# Change: async-http-serving

## Why
Sengoo can now serve dynamic HTTP requests through a pull-based
`HttpServer.next_request(timeout_ms)` API, and the async runtime has reactor
wakeups, timers, user futures, and bounded select/timeout helpers. The missing
mainstream step is an async serving loop that lets user code write:

```sg
while true {
    let outcome = await server.next_request_async(5000);
    if outcome.is_ok {
        outcome.value.respond(200, "ok");
    }
}
```

without blocking the cooperative runtime thread while waiting for the next
client. Go, Python, Node, Rust, and TypeScript all have a default path for this
shape; Sengoo currently requires either synchronous serial polling or external
process orchestration.

## What Changes
- Add a reactor-backed async request future for `std::net::HttpServer`.
- Preserve the existing synchronous `next_request(timeout_ms)` and
  `serve_once` behavior unchanged.
- Reuse the current request handle, introspection, response, limit, fallback,
  and `Connection: close` semantics.
- Add timeout/cancellation-safe cleanup rules for a pending async accept future.
- Add compiler/runtime/stdlib/LSP/realworld tests that prove an async Sengoo
  server can answer a real localhost request through `sgc`/`sgpm`.

## Scope
### In Scope
- Plain HTTP/1.1 server accept/read readiness on supported native hosts.
- `HttpServer.next_request_async(timeout_ms) -> Future<HttpServerNextRequestOutcome>`
  following the existing `Future<T>::poll` contract.
- `await server.next_request_async(...)` examples and diagnostics.
- C-only fallback symbols returning `STATUS_UNSUPPORTED`.
- Support matrix updates that mark async HTTP serving as a supported subset
  only where native runtime support is proven.

### Out of Scope
- TLS/HTTPS server termination.
- Keep-alive, pipelining, HTTP/2, streaming request/response bodies.
- Runtime-owned Sengoo callback handlers.
- Multi-threaded accept pools.
- General task cancellation or select loser cancellation beyond cleanup needed
  for the async accept future.

## Impact
- Specs:
  - `stdlib-http-server`
  - `async-default-followups`
- Code likely touched:
  - `runtime/src/net/http_server.rs`
  - `runtime/src/async_runtime.rs`
  - `runtime/src/async_runtime/reactor.rs`
  - `runtime/src/net.rs`
  - `tools/stdlib/net.sg`
  - `tools/stdlib/async*.sg` if wrapper shape requires a helper
  - `tools/stdlib/runtime.c` and/or `runtime_breadth.c` fallback stubs
  - `tools/sgc/src/tests.rs`
  - `tools/sglsp/src/stdlib.rs`
  - `tools/sgpm/tests/realworld_e2e.rs`
  - `examples/realworld/*`
  - `examples/realworld/SUPPORT_MATRIX.md`
- Compatibility: additive only. Existing HTTP server and async APIs keep their
  source and runtime behavior.

## Success Criteria
- A Sengoo realworld package can start an async HTTP server, await a request,
  answer it, and pass the locked `sgpm update/check/test/fmt --check/doc/build`
  loop.
- Native tests prove pending accept futures wake on inbound localhost
  connections, time out deterministically, and release interest/handles when
  dropped.
- `openspec validate async-http-serving --strict` and
  `openspec validate --all --strict` pass.
