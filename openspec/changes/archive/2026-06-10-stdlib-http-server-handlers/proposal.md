## Why

Sengoo can already make HTTP/HTTPS client calls and can stand up a
fixed-response test server (`bind` + static `add_route` + `serve_once`), but it
still cannot express a real web service: there is no way to run user Sengoo
code per request, no request introspection, and no sustained serve loop. This
is the largest remaining application-surface gap versus mainstream usable
languages (Go, Python, Node all serve dynamic HTTP out of the box), and it
blocks the core "migrate Python services gradually" positioning.
`stdlib-mainstream-usability` explicitly requires broader server APIs to arrive
through a dedicated OpenSpec change; this is that change.

## What Changes

- Add a pull-based dynamic request loop to `std::http`: `server_next_request`
  yields a request handle; user code computes and sends a response with
  `server_respond`. No cross-FFI user callbacks in v1.
- Add request introspection helpers on the request handle: method, path,
  query string, selected header read, and bounded body access (Buffer and
  owned-`String` variants following existing stdlib conventions).
- Add response construction: status code, bounded body, and a small set of
  explicit response headers (content-type first); unanswered requests get a
  deterministic `504`-style fallback on handle close.
- Add a sustained serve lifecycle: keep `serve_once` source-compatible, add
  bounded `serve_until_idle`-style draining on top of the same accept loop, and
  an explicit stop/close path that answers in-flight requests deterministically.
- Keep existing static routes, require-header middleware, and ws-echo routes
  source-compatible; static routes answer before the dynamic queue sees the
  request.
- Enforce existing resource limits (`set_limits` header/body caps) plus new
  explicit pending-request and per-request read/write timeout bounds; failures
  map to stable `std::status` codes.
- C-only fallback bundle keeps returning `STATUS_UNSUPPORTED` for all new
  symbols (same policy as the existing server subset).

## Capabilities

### New Capabilities

- `stdlib-http-server`: dynamic HTTP request serving in `std::http` — pull-based
  request loop, request introspection, response construction, serve lifecycle,
  resource bounds, and status mapping. Owns the existing static-route subset
  rows as its baseline.

### Modified Capabilities

<!-- none: stdlib-mainstream-usability's expansion-gate requirement is being
followed, not changed; its requirement text stays as-is. -->

## Impact

- `runtime/src/net/http_server.rs` (request queue, respond path, serve loop),
  `runtime/src/net.rs` exports.
- `tools/stdlib/net.sg` extern list and `tools/stdlib/http.sg` public wrappers
  (`HttpServer`, `HttpServerRequest` style handles).
- `tools/stdlib/runtime_breadth.c` unsupported stubs for the new symbols.
- `tools/sglsp` stdlib signatures/completions; `tools/sgc` stdlib tests.
- `examples/realworld`: new dynamic HTTP service fixture; `SUPPORT_MATRIX.md`
  HTTP server row moves from implicit/unlisted to explicit supported-subset.
- Docs: `docs/network-runtime.md` server section.
- No TLS server, no streaming bodies, no async-await integration in this change
  (async serve belongs to a later reactor-backed change).
