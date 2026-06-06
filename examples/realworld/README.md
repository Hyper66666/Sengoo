# Realworld Package Loop

These fixtures are committed Sengoo packages for checking the package-manager
workflow against project-shaped code instead of isolated snippets.

Run the locked loop from the repository root by entering one fixture directory:

```powershell
cd examples/realworld/cli-json-audit
sgpm update
sgpm check --locked
sgpm test --locked
sgpm fmt --check --locked
sgpm doc --locked
sgpm build --locked
```

Repeat the same command sequence from:

```powershell
cd examples/realworld/http-client-status
```

```powershell
cd examples/realworld/workspace-doc-loop
```

Packages:

- `cli-json-audit`: CLI-style data audit using args, file, dir, json, log,
  status, and collections helpers.
- `http-client-status`: HTTP/status example using the public `std::http`
  wrapper and a stable unsupported-scheme path.
- `workspace-doc-loop`: dual-target package with a library entry, package
  tests, docs, and process invocation.

Use [SUPPORT_MATRIX.md](SUPPORT_MATRIX.md) as the support and gap reference for
runtime, stdlib, package, doc, test, and LSP behavior.
