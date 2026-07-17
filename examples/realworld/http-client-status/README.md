# http-client-status

`http-client-status` demonstrates the public `std::http` client surface without
depending on external network availability.

- `ftp://` remains unsupported and maps to `STATUS_UNSUPPORTED`.
- This fixture intentionally does not assert local `https://127.0.0.1/` failure
  categories because hosts without a listener can fail before TLS negotiation
  begins.
- HTTPS/TLS status behavior and verified HTTPS success are covered by
  `cargo test -p sengoo-runtime net::tls -- --nocapture`. POSIX hosts inject a
  test CA through the rustls test root-store hook and must exercise:
  `tls_success_with_test_ca_root`,
  `tls_hostname_mismatch_maps_to_hostname_error`,
  `tls_untrusted_certificate_maps_to_cert_invalid`, and
  `https_get_runtime_roundtrip_smoke`. These tests use certificate and hostname
  verification; they do not use `verify=false`, fake TLS, or a plaintext
  fallback.
- On Windows, the same command covers Schannel negative/status behavior only.
  This workspace cannot execute `#[cfg(not(windows))]` tests; a cross-target
  compile attempt for `x86_64-unknown-linux-gnu` is also blocked unless the host
  provides a Linux C toolchain/sysroot for `ring` (for example
  `x86_64-linux-gnu-gcc` or an equivalent clang sysroot).

```powershell
sgpm update
sgpm check --locked
sgpm test --locked
sgpm fmt --check --locked
sgpm doc --locked
sgpm build --locked
sgpm run --locked
```

HTTP/TLS support claims are tracked in `../SUPPORT_MATRIX.md`.
