# http-client-status

`http-client-status` demonstrates the public `std::http` client surface without
depending on external network availability. The package uses an unsupported
`ftp://` URL to assert the stable `STATUS_UNSUPPORTED` path.

```powershell
sgpm update
sgpm check --locked
sgpm test --locked
sgpm fmt --check --locked
sgpm doc --locked
sgpm build --locked
```

HTTP/TLS support claims are tracked in `../SUPPORT_MATRIX.md`.
