# cli-json-audit

`cli-json-audit` is a small package-shaped data audit. It keeps the runtime
work local and deterministic while exercising CLI args, file and directory IO,
JSON parsing, logging, status names, and collections.

```powershell
sgpm update
sgpm check --locked
sgpm test --locked
sgpm fmt --check --locked
sgpm doc --locked
sgpm build --locked
sgpm run --locked
```

The committed sample data lives under `data/`.
