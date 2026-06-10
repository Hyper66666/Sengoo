## TLS backend

- Windows: Schannel via the `native-tls` crate (uses OS Schannel APIs and the
  Windows system trust store).
- POSIX: **rustls** with the `ring` crypto provider and
  **`rustls-native-certs`** loading the host platform trust anchors into a
  `RootCertStore` at connect time. No OpenSSL runtime dependency.
- Dependency rationale:
  - `native-tls` on Windows avoids re-implementing Schannel while keeping verified
    TLS aligned with the OS trust store.
  - `rustls` + `rustls-native-certs` on POSIX keeps the runtime self-contained in
    Rust, loads the same host trust anchors users expect, and allows unit tests to
    inject a test CA through a test-only root-store hook without `verify=false`.
    The `ring` rustls feature is enabled explicitly so production POSIX clients
    have a concrete crypto provider instead of relying on test-only feature
    unification.
- Adding a new third-party dependency requires recording the dependency rationale
  in this change before code lands.
- The spec requires verified TLS, not a specific library name.

## API

- Existing `http_client_get` / `http_client_post` accept `https://` URLs unchanged.
- New status codes:
  - `STATUS_TLS_CERT_INVALID = 15`
  - `STATUS_TLS_HOSTNAME_MISMATCH = 16`
  - `STATUS_TLS_HANDSHAKE = 17`
  - `STATUS_TLS_UNAVAILABLE = 18`
- No `verify=false` in v1; insecure mode deferred.

## Limits

- Client-only TLS in v1 (no server listen/TLS terminate).
- HTTP/1.1 over TLS only; HTTP/2 deferred.
- POSIX `connect_tls` completes the rustls handshake before returning so
  certificate and hostname failures map to `STATUS_TLS_*` instead of falling
  through later HTTP request I/O as `STATUS_IO`.

## Verification

- Unit tests with a local TLS test server fixture trusted through a test-specific
  root-store/host trust setup, plus untrusted and hostname-mismatch negative
  cases. If the host cannot install or load a test trust root, the skip must name
  the missing capability and still run negative unsupported-scheme tests.
- Success tests must not disable certificate or hostname verification.
- `examples/realworld/http-client-status` gains HTTPS path or documented CI skip.

## Reference-Host Evidence Policy

This change cannot archive on Windows-only proof. Archive requires one of:

- A POSIX/reference-host run where `cargo test -p sengoo-runtime net::tls --
  --nocapture` covers trusted success, hostname mismatch, untrusted certificate,
  and HTTPS runtime roundtrip; or
- An evidenced POSIX skip that names the unavailable host capability, the exact
  skipped tests, and the follow-up required to convert the skip into a pass.

The trusted-success path must use a certificate chain trusted through the host
or a test-only injected root store. It must not use `verify=false`, hostname
verification bypass, a fake TLS transport, or a plain HTTP fallback.

CI or manual release notes must record the host triple, TLS backend, trust-store
source, and command output summary for each reference-host proof.
