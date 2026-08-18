## 1. Pinning and prerequisites

- [x] 1.1 Run `openspec validate http-production-serving --strict`.
- [x] 1.2 Confirm `async-cancellation-semantics` is archived (teardown
  primitives available); otherwise record it as the active blocker.
- [x] 1.3 Pin numeric bounds in `design.md`: max requests per connection,
  idle timeout default, max streaming chunk size.

## 2. Sengoo-side router (handlers)

- [x] 2.1 Stdlib `HttpRouter`: `http_router_new`, `http_router_route(method,
  path, handler)`, `http_router_default(handler)`, and `serve_http(server,
  router)`. Handler type frozen as
  `fn(&mut HttpServerRequest) -> Result<bool, i64>`. Method/path matched as
  exact bytes (no patterns/normalization/decoding). One default max.
  Descriptor-backed `Vec<HttpRoute>` storage removes the former four-route
  limit while keeping all function-pointer dispatch in Sengoo.
- [x] 2.2 `serve_http` is implemented in Sengoo by pulling via the existing
  async pull API (no Rust-to-Sengoo callback ABI). Unmatched routes answer 404;
  pull vs router mix rejected with stable status.
  Runtime handles are copied before suspension so async frames do not retain
  pointers into a caller poll stack.
- [x] 2.3 Handler `Err` / `Ok(false)` / unanswered maps to 500 if not already
  answered; tests cover matched, unmatched (404), failing (500), and mode
  violation.
  `stdlib_http_router_dual_routes_404_and_500_localhost` covers `Err`,
  `Ok(false)`, and `Ok(true)` without an answer independently.
- [x] 2.4 Realworld fixture serves at least two exact routes through the
  router via real `sgc` localhost smoke.

## 3. Keep-alive

- [x] 3.1 Opt-in server config with pinned bounds; HTTP/1.1 reuse semantics
  (client `Connection: close`, bound breach, or error closes).
- [x] 3.2 Runtime tests: N sequential requests on one connection; request-
  cap breach closes after in-flight response; idle timeout closes; 503
  pending-cap and drain-on-close still hold under keep-alive.
  Dedicated `keep_alive_request_cap_closes_after_final_response` and
  `keep_alive_idle_bound_expires_server_owned_connection` regressions cover
  both pinned bounds; the async path also retains an idle connection between
  cooperative poll slices.
- [x] 3.3 Default-path regression: all existing close-mode tests pass
  unchanged with keep-alive not enabled.

## 4. Streaming response bodies

- [x] 4.1 Begin/write-chunk/finish handler API with bounded chunks;
  `Content-Length` vs chunked selection per design D-C3.
- [x] 4.2 Disconnect/timeout during streaming maps to stable statuses and
  closes without breaking answer accounting.
  (IO/timeout map through existing classify_io_error; Drop/close aborts.)
- [x] 4.3 Composition tests: finished stream may keep the connection alive
  within bounds; aborted stream closes.
  The verified-CA real-`sgc` e2e sends chunked `tls-stream` then a second
  routed response over the same TLS connection; runtime tests retain oversize,
  Drop-abort, and fixed-length enforcement.

## 5. TLS server subset

- [x] 5.1 Server certificate/key configuration via stdlib config; accept
  path on Schannel (Windows) and rustls (POSIX).
  (`sengoo_http_server_bind_tls` + `http_server_bind_tls` Buffer API;
  Windows `Identity::from_pkcs8` / `TlsAcceptor`; POSIX rustls `ServerConfig`.)
- [x] 5.2 Real-handshake test with the test CA per stack on at least one
  host; failures map to `STATUS_TLS_*`; no plaintext-fallback success.
  Windows Schannel proven with explicit test-root trust and `localhost` SAN
  verification (`http_server_tls_router_keep_alive_and_streaming_compose` and
  real-`sgc` curl e2e). Empty/garbage PEM reject maps to `STATUS_TLS_*`.
- [x] 5.3 If a stack cannot be proven on an available host, record the row
  as platform-specific with the blocking reason (do not claim it).
  POSIX rustls implemented but not executed on this Windows workstation;
  matrix row is Platform-specific (not Supported).
- [x] 5.4 TLS composes with handlers, keep-alive, and streaming in at least
  one end-to-end test on a proven host.
  `http_server_tls_router_keep_alive_and_streaming_compose` plus
  `real_sgc_tls_router_keep_alive_streaming_composes_with_verified_ca` prove
  router, keep-alive, and chunked streaming over one verified-CA TLS
  connection.

## 6. Docs and matrix

- [x] 6.1 Update server docs (`docs/` runtime/network pages) for handlers,
  keep-alive bounds, streaming, and TLS server configuration.
- [x] 6.2 Update the SUPPORT_MATRIX serving row(s) with the new supported
  subsets and proof links; keep deferred items (HTTP/2, request streaming)
  explicit.

## 7. Verification

- [x] 7.1 `cargo fmt --check` (runtime crate after fmt)
- [x] 7.2 `cargo test -p sengoo-runtime --lib --features native-bridge net -- --test-threads=1`
  (45 passed including verified-CA TLS/router/keep-alive/streaming on Windows.)
- [x] 7.3 `cargo test -p sgc stdlib_http_server_async_awaits` localhost smoke green
  Residual: full `cargo test -p sgc` suite not re-run end-to-end in this session.
- [x] 7.4 Realworld fixture locked loop (`sgpm test --locked` etc.) green.
  `realworld_locked_loop_uses_real_toolchain_binaries` passed locally across
  the reviewed fixture set; PR #51 retains the earlier four-host installed
  loop.
- [x] 7.5 `openspec validate http-production-serving --strict`

## Archive Gate

- [x] `openspec validate http-production-serving --strict` passes.
- [x] Handlers, keep-alive, and streaming are proven by runtime tests plus
  a realworld fixture through real `sgc`.
- [x] TLS server has a real-handshake proof per claimed stack; unproven
  stacks are recorded platform-specific, not claimed.
  (Windows proven; POSIX residual as Platform-specific.)
- [x] Existing pull/static/bounds/drain/fallback requirements remain green
  and unchanged. (runtime net suite 45/45)
- [x] Matrix updated with proof; umbrella records Pillar C completion.
  Current-SHA cross-host proof remains part of the release-closure matrix, not
  an unimplemented HTTP owner task.
