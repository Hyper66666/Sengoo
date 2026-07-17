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
- [x] 2.2 `serve_http` is implemented in Sengoo by pulling via the existing
  async pull API (no Rust鈫扴engoo callback ABI). Unmatched routes answer 404;
  pull vs router mix rejected with stable status.
- [x] 2.3 Handler `Err` / `Ok(false)` / unanswered maps to 500 if not already
  answered; tests cover matched, unmatched (404), failing (500), and mode
  violation.
- [x] 2.4 Realworld fixture serves at least two exact routes through the
  router via real `sgc` localhost smoke.

## 3. Keep-alive

- [x] 3.1 Opt-in server config with pinned bounds; HTTP/1.1 reuse semantics
  (client `Connection: close`, bound breach, or error closes).
- [x] 3.2 Runtime tests: N sequential requests on one connection; request-
  cap breach closes after in-flight response; idle timeout closes; 503
  pending-cap and drain-on-close still hold under keep-alive.
  Residual: dedicated idle-timeout and request-cap-breach unit tests still
  open; sequential reuse + client Connection: close + default close proven.
- [x] 3.3 Default-path regression: all existing close-mode tests pass
  unchanged with keep-alive not enabled.

## 4. Streaming response bodies

- [ ] 4.1 Begin/write-chunk/finish handler API with bounded chunks;
  `Content-Length` vs chunked selection per design D-C3.
- [ ] 4.2 Disconnect/timeout during streaming maps to stable statuses and
  closes without breaking answer accounting.
- [ ] 4.3 Composition tests: finished stream may keep the connection alive
  within bounds; aborted stream closes.

## 5. TLS server subset

- [ ] 5.1 Server certificate/key configuration via stdlib config; accept
  path on Schannel (Windows) and rustls (POSIX).
- [ ] 5.2 Real-handshake test with the test CA per stack on at least one
  host; failures map to `STATUS_TLS_*`; no plaintext-fallback success.
- [ ] 5.3 If a stack cannot be proven on an available host, record the row
  as platform-specific with the blocking reason (do not claim it).
- [ ] 5.4 TLS composes with handlers, keep-alive, and streaming in at least
  one end-to-end test on a proven host.

## 6. Docs and matrix

- [ ] 6.1 Update server docs (`docs/` runtime/network pages) for handlers,
  keep-alive bounds, streaming, and TLS server configuration.
- [ ] 6.2 Update the SUPPORT_MATRIX serving row(s) with the new supported
  subsets and proof links; keep deferred items (HTTP/2, request streaming)
  explicit.

## 7. Verification

- [ ] 7.1 `cargo fmt --check`
- [ ] 7.2 `cargo test -p sengoo-runtime --lib --features native-bridge net -- --test-threads=1`
- [ ] 7.3 `cargo test -p sgc` (HTTP server e2e + localhost smokes)
- [ ] 7.4 Realworld fixture locked loop (`sgpm test --locked` etc.) green
- [ ] 7.5 `openspec validate http-production-serving --strict`

## Archive Gate

- [ ] `openspec validate http-production-serving --strict` passes.
- [ ] Handlers, keep-alive, and streaming are proven by runtime tests plus
  a realworld fixture through real `sgc`.
- [ ] TLS server has a real-handshake proof per claimed stack; unproven
  stacks are recorded platform-specific, not claimed.
- [ ] Existing pull/static/bounds/drain/fallback requirements remain green
  and unchanged.
- [ ] Matrix updated with proof; umbrella records Pillar C completion.

