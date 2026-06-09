## Why

Internal and external HTTP clients expect `https://` as the default transport.
Sengoo `std::http` now has an implemented HTTPS/TLS supported subset on Windows
and a POSIX rustls path, but the change remains open because POSIX/reference-host
trusted-success evidence is still required before the support matrix can make a
portable default claim.

## What Changes

- Add TLS-backed `https://` client support in `std::http`.
- Freeze certificate verification policy (system trust store, no insecure skip by default).
- Map TLS failures to stable `std::status` codes.
- Update realworld example to exercise HTTPS against a test fixture or documented skip.

## Capabilities

### New Capabilities

- None.

### Modified Capabilities

- `stdlib-mainstream-usability`: HTTPS client scope, TLS error mapping, matrix row.

## Impact

- `tools/stdlib/http.sg`, `runtime` net/TLS bridge, `examples/realworld`
- Parent umbrella: `mainstream-production-readiness` Block 2
