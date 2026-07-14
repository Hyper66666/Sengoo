# compressed-json-artifact

`compressed-json-artifact` exercises the public `std::compress` gzip Buffer
APIs against a JSON payload, then parses the decompressed bytes through
`std::json`.

```powershell
sgpm update
sgpm check --locked
sgpm test --locked
sgpm fmt --check --locked
sgpm doc --locked
sgpm build --locked
sgpm run --locked
```

Compression support claims are tracked in `../SUPPORT_MATRIX.md`.
