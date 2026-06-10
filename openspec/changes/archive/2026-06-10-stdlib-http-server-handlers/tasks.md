## 1. Preparation

- [x] 1.1 Run `openspec validate stdlib-http-server-handlers --strict`.
- [x] 1.2 Re-read `runtime/src/net/http_server.rs` serve path and confirm the
  pinned API table in `design.md` (D4) still matches the extern naming and
  Buffer length+copy conventions; update the table before code edits if not.
  Confirmed: length+copy pairs follow `sengoo_http_body_len`/`body_copy`
  (`copy_bytes_to_buffer`), handles via `NetRuntime::alloc_handle`, errors via
  `fail_*` + `classify_io_error`, accept via existing `accept_with_timeout`.

## 2. Runtime implementation (native)

- [x] 2.1 Add request-handle state to `NetRuntime`: generation-checked table of
  pulled requests owning parsed request + `TcpStream`; add the pending cap
  constant (64) per `design.md` D5.
- [x] 2.2 Implement `sengoo_http_server_next_request(handle, timeout_ms)`:
  accept within timeout budget, run middleware/static/ws-echo inline first
  (D3), surface only unmatched requests, map expiry to timeout status.
- [x] 2.3 Implement request introspection externs: method/path/query/version
  and single header lookup as length+copy pairs, plus `body_len`/`body_copy`;
  split request-target at the first `?` with no percent-decoding.
- [x] 2.4 Implement `respond`, `respond_with_content_type`, and `close` with
  exactly-once semantics: write + flush + close + free on respond; `504`
  fallback on unanswered close; handle-invalid error on double respond;
  response body capped by `max_body_bytes`.
- [x] 2.5 Implement pending-cap overflow answering `503` inline and server
  `close` draining all queued/pulled-unanswered requests with the `504`
  fallback, leaving handle tables empty.
- [x] 2.6 Add runtime unit/integration tests with a client thread covering:
  pull/respond roundtrip, timeout mapping, static-route precedence, header
  lookup case-insensitivity and absent-header status, double respond, close
  fallback, pending-cap overflow, oversized response rejection, and
  close-drain leaving tables empty (`runtime/src/net.rs` tests; close-drain
  also asserts the global request handle table is empty).

## 3. C fallback bundle

- [x] 3.1 Add all new symbols to `tools/stdlib/runtime_breadth.c` returning the
  existing unsupported/handle-not-found fallback statuses.
- [x] 3.2 Extend the `sgc` hardening test that asserts C-bundle stub mappings
  to cover the new symbols
  (`runtime_hardening_c_bundle_http_server_request_symbols_map_fallback_statuses`
  drives every new wrapper through the C-only link path and asserts
  `STATUS_UNSUPPORTED` / `STATUS_INVALID_HANDLE`).

## 4. Stdlib surface

- [x] 4.1 Add externs to `tools/stdlib/net.sg` and public wrappers for
  `HttpServer.next_request(timeout_ms)` and `HttpServerRequest` with
  `method_string`, `path_string`, `query_string`, `version_string`,
  `header_string(name)`, `body_len`, `body_copy(buffer)`,
  `respond(status, body)`, `respond_with_content_type(...)`, `close()`.
  Deviation from the original task text: the wrappers live in
  `tools/stdlib/net.sg` next to the existing `HttpServer` impl instead of
  `tools/stdlib/http.sg`, which remains the client-only surface; this keeps
  the server wrapper family in one module.
- [x] 4.2 Follow owned-`String` helper conventions for `_string` getters and
  `Result<T, i64>` status returns; keep existing `serve_once` wrappers
  untouched.
- [x] 4.3 Add `cargo test -p sgc stdlib_http` coverage compiling a Sengoo
  program that binds, pulls one localhost request, inspects method/path/query,
  and responds; assert client-visible bytes
  (`stdlib_http_server_pulls_and_answers_localhost_request` spawns the
  compiled server, sends a real TCP request, and asserts the echoed bytes;
  `stdlib_net_import_preloads_http_server_request_wrappers` covers the IR
  surface).

## 5. Toolchain, examples, docs

- [x] 5.1 Add `sglsp` stdlib signatures/completions for the new server items
  and extend `cargo test -p sglsp stdlib` accordingly
  (`stdlib_symbols_follow_http_server_request_surface`; the `net` module's
  LSP dependency set now includes `string` so owned-`String` getters resolve).
- [x] 5.2 Add `examples/realworld/http-echo-service` fixture: dynamic echo
  service with a smoke test through the locked `sgpm` loop; wire it into the
  realworld e2e list. Scope note: a single synchronous Sengoo process cannot
  be its own HTTP client, so the committed smoke test proves the
  network-independent subset (bind, `next_request` timeout mapping, stale
  handle mapping, clean close) and the real serve-and-answer roundtrip is
  proven by the `sgc` e2e test
  `stdlib_http_server_pulls_and_answers_localhost_request` (real `sgc` build
  answering a real localhost TCP client).
- [x] 5.3 Document the dynamic serving subset (serial, plaintext,
  `Connection: close`, pending cap, no percent-decoding) in
  `docs/network-runtime.md`.
- [x] 5.4 Add the HTTP server row to `examples/realworld/SUPPORT_MATRIX.md`
  with status, host scope, proof commands, and stable diagnostics.

## 6. Verification

All run locally on the Windows workstation (x86_64-pc-windows-msvc) on
2026-06-11:

- [x] 6.1 `cargo fmt --check`
- [x] 6.2 `cargo test -p sengoo-runtime net -- --test-threads=1` (32 passed;
  also `--lib` full suites: default 62 passed, `--features native-bridge`
  65 passed)
- [x] 6.3 `cargo test -p sgc stdlib_http` (2 passed; `cargo test -p sgc --bin
  sgc stdlib` 108 passed; `runtime` filter 104 passed; `async_native_runtime`
  36 passed)
- [x] 6.4 `cargo test -p sglsp stdlib` (26 passed)
- [x] 6.5 `cargo test -p sgpm realworld_locked_loop_uses_real_toolchain_binaries --test realworld_e2e -- --nocapture`
  (all seven fixtures including `http-echo-service`)
- [x] 6.6 `cargo clippy -p sengoo-runtime -p sgc -p sglsp --all-targets -- -D warnings`
- [x] 6.7 `openspec validate stdlib-http-server-handlers --strict`

## Implementation notes (defects found and fixed while landing)

- Windows hang: `TcpListener::try_clone()` does not preserve the non-blocking
  flag on Windows, so the snapshot listener blocked forever inside
  `accept`; `next_request` with no client hung indefinitely (reproduced as
  stuck test processes). `http_server_snapshot` now re-applies
  `set_nonblocking(true)`, and `accept_with_timeout` restores blocking mode
  on accepted streams so read/write timeouts apply (accepted sockets inherit
  the listener's non-blocking mode on Windows).
- Native net linkage: the `native-bridge` staticlib previously excluded the
  whole `net` module, so every `sengoo_http_*`/`sengoo_tcp_*` symbol in
  compiled Sengoo programs silently resolved to the C fallback stubs and the
  dynamic server API could never work through `sgc build`. The staticlib now
  ships the real `net` module; the C net stubs compile out via
  `-DSENGOO_NATIVE_NET_RUNTIME` on the native link path (object definitions
  always beat archive members); `reflect` stays excluded because its C twins
  live in `runtime.c`; the net-bench stubs stay unconditional for the same
  reason.
- Archive extraction fallout: a dedicated `[profile.staticlib]` (no LTO,
  per-module codegen units) keeps staticlib members independently
  extractable, and `tools/stdlib/runtime.c` now provides benign fallbacks for
  the async program-side contract (`main__*`,
  `sengoo_async_*_dispatch*`) via `/alternatename` on Windows and weak
  definitions on POSIX, so non-async programs that pull net members no longer
  fail with unresolved async externs. Async programs still override them with
  their compiled IR definitions (async suite stays green).
- ABI widening: `sengoo_net_last_error`, `sengoo_net_bench_last_error_code`,
  and `sengoo_net_bench_last_error_clear` now return `i64` to match the
  stdlib extern declarations on every architecture (negative bench codes
  would otherwise zero-extend incorrectly).
- The MSVC link path adds `advapi32/bcrypt/crypt32/ncrypt/secur32` for the
  schannel-backed TLS client inside the staticlib.

## Archive Gate

- [x] `openspec validate stdlib-http-server-handlers --strict` passes.
- [x] A Sengoo realworld fixture serves a dynamic localhost request end to end
  with real `sgc`/`sgpm`, or the blocking host capability is recorded here
  with evidence. Evidence: `examples/realworld/http-echo-service` passes the
  locked loop (network-independent smoke), and
  `stdlib_http_server_pulls_and_answers_localhost_request` proves the real
  serve-and-answer roundtrip through a real `sgc`-built server answering a
  real localhost TCP client (single-process Sengoo cannot self-serve in the
  synchronous pull model; see task 5.2 scope note).
- [x] Exactly-once answering is proven by tests (double respond rejected,
  unanswered close sends `504`, server close drains to empty tables —
  `http_request_double_respond_is_rejected`,
  `http_request_close_unanswered_sends_gateway_timeout`,
  `http_server_close_drains_unanswered_requests_with_gateway_timeout`).
- [x] C-only bundle returns `STATUS_UNSUPPORTED` for every new symbol
  (`next_request` maps to `STATUS_UNSUPPORTED`; per-request handle symbols map
  to `STATUS_INVALID_HANDLE`, matching the existing server-subset policy).
- [x] Existing static-route/middleware/ws-echo behavior is unchanged
  (`serve_once` tests stay green; full `net` suite 32 passed).
- [x] `SUPPORT_MATRIX.md` HTTP server row landed with proof links.
