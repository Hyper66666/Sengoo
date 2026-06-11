# Tasks: async-http-serving

## 1. Spec Gate
- [x] 1.1 Run `openspec validate async-http-serving --strict`.
- [x] 1.2 Re-read `openspec/specs/stdlib-http-server/spec.md`,
  `openspec/specs/async-default-followups/spec.md`,
  `runtime/src/net/http_server.rs`, and `tools/stdlib/net.sg`.
- [x] 1.3 Confirm the public API name and future shape in implementation notes
  before touching code; update this change if a compiler limitation requires a
  different concrete wrapper name.

## 2. Runtime Reactor Integration
- [x] 2.1 Add failing native tests for async HTTP next-request timeout without
  tearing down the server.
- [x] 2.2 Add failing native tests for listener-read wakeup when a localhost
  client connects.
- [x] 2.3 Add failing native tests that dropping a pending async request future
  unregisters readiness interest and leaves the server usable.
- [x] 2.4 Factor the existing synchronous HTTP accept/read/route/middleware
  flow so sync and async paths share request parsing, static-route precedence,
  middleware rejection, ws-echo routing, dynamic handle creation, and status
  mapping.
- [x] 2.5 Implement the pollable async next-request operation on native hosts.
- [x] 2.6 Ensure accepted-but-unpublished slow-client timeout paths answer or
  close deterministically without surfacing partial request handles.

## 3. C ABI And Fallback Bundle
- [x] 3.1 Add native bridge symbols needed by the stdlib future wrapper.
- [x] 3.2 Add C-only fallback stubs returning `STATUS_UNSUPPORTED`.
- [x] 3.3 Extend runtime hardening tests so missing native symbols do not appear
  in staticlib/C-bundle builds.

## 4. Stdlib Surface
- [x] 4.1 Add `HttpServer.next_request_async(timeout_ms)` and its concrete
  future wrapper to `tools/stdlib/net.sg`.
- [x] 4.2 Preserve existing `next_request(timeout_ms)` behavior and signatures.
- [x] 4.3 Map timeout, invalid handle, unsupported host, and IO/protocol errors
  to the same status taxonomy as the synchronous wrapper.
- [x] 4.4 Add examples or comments only where needed; avoid introducing a
  callback-style API in this change.

## 5. Compiler, SGC, And LSP Tests
- [x] 5.1 Add `sgc` tests that compile and run an async HTTP server answering a
  real localhost request.
- [x] 5.2 Add a negative `sgc`/compiler diagnostic test for awaiting
  synchronous `next_request` directly, unless such a test already exists.
- [x] 5.3 Add `sglsp` symbols/signatures/completions for the async server
  surface.
- [x] 5.4 Verify existing async user-future tests still pass.

## 6. Realworld And Docs
- [x] 6.1 Add or extend a realworld fixture so the locked package loop proves
  async HTTP serving with real `sgc`/`sgpm` binaries.
- [x] 6.2 Update `examples/realworld/SUPPORT_MATRIX.md` with an async HTTP
  server row scoped to proven hosts and deferred TLS/keep-alive/streaming.
- [x] 6.3 Update `docs/network-runtime.md` with async serving semantics,
  timeout/drop cleanup, host support, and non-goals.
- [x] 6.4 Keep existing HTTP server fixture docs source-compatible.

## 7. Verification
- [x] 7.1 `cargo fmt --check`
- [x] 7.2 `cargo test -p sengoo-runtime net -- --test-threads=1`
- [x] 7.3 `cargo test -p sengoo-runtime --lib --features native-bridge async -- --test-threads=1`
- [x] 7.4 `cargo test -p sgc stdlib_http -- --nocapture`
- [x] 7.5 `cargo test -p sgc async_native_runtime -- --nocapture --test-threads=1`
- [x] 7.6 `cargo test -p sglsp stdlib -- --nocapture`
- [x] 7.7 `cargo test -p sgpm realworld_locked_loop_uses_real_toolchain_binaries --test realworld_e2e -- --nocapture`
- [x] 7.8 `cargo clippy -p sengoo-runtime -p sgc -p sglsp --all-targets -- -D warnings`
- [x] 7.9 `openspec validate async-http-serving --strict`
- [x] 7.10 `openspec validate --all --strict`

## 8. Archive Gate
- [x] 8.1 Confirm support matrix rows cite proof commands and mark deferred
  TLS server, keep-alive, streaming bodies, callback handlers, and broad
  cancellation semantics.
- [x] 8.2 Archive only after native and realworld proof pass on the host set
  claimed by the docs.
