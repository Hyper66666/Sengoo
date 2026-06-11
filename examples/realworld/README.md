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
cd examples/realworld/async-channel-smoke
```

```powershell
cd examples/realworld/compressed-json-artifact
```

```powershell
cd examples/realworld/http-client-status
```

```powershell
cd examples/realworld/http-echo-service
```

```powershell
cd examples/realworld/package-release-loop
```

```powershell
cd examples/realworld/workspace-doc-loop
```

Packages:

- `async-channel-smoke`: async package smoke using public `std::async`
  channel/mutex helpers plus cooperative `sleep`, `spawn`, and `select`.
- `cli-json-audit`: CLI-style data audit using args, file, dir, json, log,
  status, and collections helpers.
- `compressed-json-artifact`: compressed JSON artifact smoke using public
  `std::compress` gzip Buffer helpers and `std::json` parse verification.
- `http-client-status`: HTTP/status example using the public `std::http`
  wrapper and a stable unsupported-scheme path.
- `http-echo-service`: dynamic HTTP echo service using the reactor-backed
  `std::net` server subset (`await next_request_async`, request introspection,
  exactly-once `respond`), with a network-independent smoke test.
- `package-release-loop`: package release fixture covering dependency aliases,
  two selected local-registry versions of `shared_core`, deterministic publish
  metadata, local registry publish, and locked command stability.
- `workspace-doc-loop`: dual-target package with a library entry, package
  tests, docs, and process invocation.

Use [SUPPORT_MATRIX.md](SUPPORT_MATRIX.md) as the support and gap reference for
runtime, stdlib, package, doc, test, and LSP behavior.
