# Explicit request-copy limits

`std::net` keeps the existing `net_stream_start` and `net_stream_start_ca`
request limit of 1 MiB. New `net_stream_start_bounded` and
`net_stream_start_ca_bounded` append an explicit `request_limit` argument,
1 through 16777216 bytes. Both return the same `NetByteStream` and retain
the existing timeout, TLS verification, cancellation and response-queue behavior.

The request is copied before return. Live copies share a process-wide 64 MiB
admission budget, independently of the existing eight-worker/eight-handle limit.
Validation and admission occur before copying the request. Invalid limits return
status 2; aggregate capacity exhaustion returns status 13 without a new handle.
Allocation is fallible. Cancellation is immediate, but the copy budget is released
only when the owning worker actually drops its copy. For a conservative bound,
the reservation remains held throughout response streaming, after upload finishes.
`sengoo_net_stream_live_request_bytes()` exposes reserved bytes for diagnostics.

These are byte transport APIs, not an HTTP upload or provider API. Application
code must preflight the entire serialized request, including escaped JSON, base64,
headers, history and tools. Raising the explicit limit does not establish image
or PDF support. Native workers do not receive JSON, database or Agent state.

Evidence on Windows x64, 2026-11-04:

- `target/byte-stream-bounded-tests4.log`: 11 byte-stream tests pass, including
  original plain/TLS behavior, original 1 MiB rejection, validation before pointer
  access, actual larger plain/TLS copies, four simultaneous 16 MiB reservations,
  aggregate rejection before copying, cancel and failed-connect reclamation.
- `target/bounded-request-public.log`: public Sengoo native test passes through
  a private compiler output; a real peer receives a request larger than 1 MiB.
  The old entry point rejects the same bytes and the explicit entry point succeeds.

Testing exposed a handshake deadline race: native TLS could return its generic
handshake error just after the deadline but before the monitor published timeout.
The byte worker now checks the already-expired deadline on handshake failure;
no certificate or hostname validation is disabled. The original deadline test
passes. Failed/intermediate logs remain diagnostic history. Other platforms and
installed toolchain distribution of these new symbols remain separate gates.
