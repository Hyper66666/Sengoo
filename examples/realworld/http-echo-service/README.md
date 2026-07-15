# http-echo-service

`http-echo-service` demonstrates the dynamic `std::net` HTTP serving subset:
reactor-backed `await HttpServer.next_request_async`, request introspection on
`HttpServerRequest`, and exactly-once `respond`/`close` answering.

- `src/main.sg` binds `127.0.0.1:0`, awaits one dynamic request (5s budget),
  and echoes the request body back with status `200`. Timeout expiry exits
  cleanly, so the binary works both interactively (send a request with any
  HTTP client) and unattended.
- `tests/http_echo_smoke.sg` stays single-process: it spawns a localhost TCP
  client, awaits `next_request_async`, answers the request, checks stale
  request handles map to `STATUS_INVALID_HANDLE`, and closes the server
  cleanly. On C-only fallback hosts the bind itself maps to
  `STATUS_UNSUPPORTED` and the smoke records that as the supported outcome.
- The full serve-and-answer roundtrip (real localhost client, echoed bytes)
  is proven by `cargo test -p sgc stdlib_http_server_async_awaits_and_answers_localhost_request`,
  which compiles an async Sengoo server through real `sgc` and answers a real
  TCP client. The synchronous `next_request` path remains source-compatible
  and retains its own localhost integration test.

```powershell
sgpm update
sgpm check --locked
sgpm test --locked
sgpm fmt --check --locked
sgpm doc --locked
sgpm build --locked
sgpm run --locked
```

Dynamic serving limits (one request future per serial server loop, plaintext only,
`Connection: close` per request, pending cap 64, no percent-decoding) are
documented in `../../../docs/network-runtime.md`; support claims live in
`../SUPPORT_MATRIX.md`.
