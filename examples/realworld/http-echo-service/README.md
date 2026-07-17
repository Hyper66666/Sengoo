# http-echo-service

`http-echo-service` demonstrates the Sengoo-side `HttpRouter` production serving
subset: exact method+path handlers over the existing async pull API (no reverse
FFI callback ABI), dual-route registration, and exactly-once `respond`.

- `src/main.sg` binds `127.0.0.1:0`, registers `GET /health` and `GET /echo`,
  and serves one request with a 5s budget so the binary exits cleanly when run
  unattended. Send a request with any HTTP client before the budget expires to
  exercise a handler interactively.
- `tests/http_echo_smoke.sg` stays single-process: it spawns two localhost TCP
  clients against `/health` and `/echo`, dispatches both through `serve_http`,
  and closes the server cleanly. On C-only fallback hosts the bind itself maps
  to `STATUS_UNSUPPORTED` and the smoke records that as the supported outcome.
- Unmatched paths answer `404`; handlers that return `0` (not answered) map to
  `500`. Pull and router modes are exclusive on one listener.
- Full dual-route / 404 / 500 / mode-mix proof is also covered by
  `cargo test -p sgc stdlib_http_router_dual_routes_404_and_500_localhost` and
  `stdlib_http_router_rejects_pull_mode_mix`.

```powershell
sgpm update
sgpm check --locked
sgpm test --locked
sgpm fmt --check --locked
sgpm doc --locked
sgpm build --locked
sgpm run --locked
```

Dynamic serving limits (exact method+path only, plaintext only, default
`Connection: close`, pending cap 64, no percent-decoding, no reverse Sengoo
callback ABI) are documented in `../../../docs/network-runtime.md`; support
claims live in `../SUPPORT_MATRIX.md`. Keep-alive, response streaming, and TLS
server remain residual under open owner `http-production-serving`.
