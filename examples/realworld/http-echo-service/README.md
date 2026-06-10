# http-echo-service

`http-echo-service` demonstrates the dynamic `std::net` HTTP serving subset:
pull-based `HttpServer.next_request`, request introspection on
`HttpServerRequest`, and exactly-once `respond`/`close` answering.

- `src/main.sg` binds `127.0.0.1:0`, pulls one dynamic request (5s budget),
  and echoes the request body back with status `200`. Timeout expiry exits
  cleanly, so the binary works both interactively (send a request with any
  HTTP client) and unattended.
- `tests/http_echo_smoke.sg` stays single-process and network-independent: it
  proves `next_request` timeout maps to `STATUS_TIMEOUT`, stale request
  handles map to `STATUS_INVALID_HANDLE`, and server close stays clean. On
  C-only fallback hosts the bind itself maps to `STATUS_UNSUPPORTED` and the
  smoke records that as the supported outcome.
- The full serve-and-answer roundtrip (real localhost client, echoed bytes)
  is proven by `cargo test -p sgc stdlib_http_server_pulls_and_answers_localhost_request`,
  which compiles a Sengoo server through real `sgc` and answers a real TCP
  client. A single Sengoo process cannot be both client and server in the
  synchronous pull model, so the smoke test does not fake one.

```powershell
sgpm update
sgpm check --locked
sgpm test --locked
sgpm fmt --check --locked
sgpm doc --locked
sgpm build --locked
```

Dynamic serving limits (serial accept loop, plaintext only,
`Connection: close` per request, pending cap 64, no percent-decoding) are
documented in `../../../docs/network-runtime.md`; support claims live in
`../SUPPORT_MATRIX.md`.
