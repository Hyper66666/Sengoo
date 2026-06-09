## 1. Preparation

- [x] 1.1 Run `openspec validate stdlib-https-tls --strict`.

## 2. Implementation

- [x] 2.0 Choose POSIX TLS backend (OpenSSL or rustls root-store path) and record dependency/trust-store rationale in `design.md`.
- [x] 2.1 Native TLS bridge (Schannel on Windows; recorded backend on POSIX).
- [x] 2.2 Wire `https://` through `sengoo_http_get` / `post`.
- [x] 2.3 Add `STATUS_TLS_CERT_INVALID=15`, `STATUS_TLS_HOSTNAME_MISMATCH=16`, `STATUS_TLS_HANDSHAKE=17`, and `STATUS_TLS_UNAVAILABLE=18` to C runtime, `std::status`, names/messages, and net error mapping.
- [x] 2.4 Update realworld HTTP example and SUPPORT_MATRIX TLS row.

## 3. Verification

- [x] 3.1 Windows host: `cargo test -p sengoo-runtime net::tls -- --nocapture` covers untrusted certificate and backend-unavailable/no-roots behavior.
- [ ] 3.1b POSIX/reference host: `cargo test -p sengoo-runtime net::tls -- --nocapture` covers trusted success, hostname mismatch, untrusted certificate, and HTTPS runtime roundtrip (`#[cfg(not(windows))]` tests).
  - [x] Test structure exists for `tls_success_with_test_ca_root`,
    `tls_hostname_mismatch_maps_to_hostname_error`,
    `tls_untrusted_certificate_maps_to_cert_invalid`, and
    `https_get_runtime_roundtrip_smoke`; success uses a test CA injected into
    rustls roots and does not disable certificate or hostname verification.
  - [x] POSIX rustls `connect_tls` completes the handshake before returning so
    certificate/hostname failures map to `STATUS_TLS_*` instead of later HTTP
    request I/O.
  - [ ] Run the command on a POSIX/reference host and paste host triple/backend
    evidence before archive.
- [ ] 3.1c Record the POSIX host triple, TLS backend, trust-store source, and
  command output summary in this change or the support matrix.
- [x] 3.1d If POSIX proof is skipped, record the exact missing host capability
  and keep HTTPS as `Platform-specific`; do not substitute fake TLS or
  verification-disabled success. Current Windows workstation cannot execute
  `#[cfg(not(windows))]`; `cargo check -p sengoo-runtime --tests --target
  x86_64-unknown-linux-gnu` is blocked by missing Linux C toolchain/sysroot for
  `ring` (`x86_64-linux-gnu-gcc`; clang attempt fails on missing Linux
  `assert.h`). `examples/realworld/SUPPORT_MATRIX.md` keeps HTTPS
  `Platform-specific` pending the reference-host run. `.github/workflows/realworld-e2e.yml`
  includes a Linux-only `cargo test -p sengoo-runtime net::tls -- --nocapture`
  step so CI can provide the missing POSIX evidence, but this workspace has no
  remote job output to cite yet.
- [x] 3.2 `cargo test -p sgc stdlib_http` - passes after cold `sengoo-runtime` TLS staticlib build (first run may compile `native-tls`).
- [x] 3.3 `cargo test -p sglsp stdlib` covers http import signatures.
- [x] 3.4 `cargo test -p sgpm realworld_locked_loop_uses_real_toolchain_binaries --test realworld_e2e -- --nocapture` proves the realworld HTTPS/status fixture still passes or records an evidenced network/TLS skip.

## Archive Gate

- [x] `openspec validate stdlib-https-tls --strict` passes.
- [ ] HTTPS client success works on reference host or documents evidenced skip.
- [ ] TLS success tests use real certificate/hostname verification; no `verify=false` or fake TLS stubs. Test structure is present; pending POSIX/reference-host execution on this Windows workstation.
- [x] New `STATUS_TLS_*` categories are documented and exposed through `std::status`.
- [x] Plain `http://` behavior unchanged.
