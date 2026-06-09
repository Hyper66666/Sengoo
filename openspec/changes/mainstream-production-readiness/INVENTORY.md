# Baseline Inventory (mainstream-production-readiness)

## Block status (June 2026)

`mainstream-production-readiness` now tracks the current front-five blockers.
Compile scale is retained as closed historical evidence and must not reopen as a
competing blocker unless new measurements regress below the accepted gate.

| Block | Gap | Current evidence | Child change |
| --- | --- | --- | --- |
| 0 Compile scale evidence | Closed/superseded 1000k RSS/frontend-share gate | `bench/results/1780946346830-advanced-pipeline.json`, `mainstream-default-readiness/INVENTORY.md` | `compile-scale-production-gate` historical evidence |
| 1 Async defaults | User `Future::poll`, all-host owned-fd readiness, cancellation boundaries, public cleanup-wrapper lowering | `docs/runtime-async-semantics.md`, `examples/realworld/SUPPORT_MATRIX.md`, `async-default-followups/tasks.md` | `async-default-followups` |
| 2 HTTPS/TLS evidence | POSIX/reference-host trusted success, hostname mismatch, and HTTPS runtime roundtrip proof | `stdlib-https-tls/tasks.md`, `examples/realworld/http-client-status` | `stdlib-https-tls` |
| 3 Stdlib compression | Real gzip-compatible `std::compress` APIs, resource limits, deterministic gzip behavior, realworld fixture | `stdlib-default-followups/tasks.md`, `tools/stdlib/README.md`, `examples/realworld/SUPPORT_MATRIX.md` | `stdlib-default-followups` |
| 4 Language polish | Pinned cfg predicates, FFI rejection parity, payload-enum async frame decision, match/try diagnostic parity | `language-default-polish/tasks.md`, compiler/sglsp diagnostics tests | `language-default-polish` |
| 5 Package/release defaults | Deterministic publish artifacts, local/remote registry publish evidence, release fixture, release smoke/rollback docs | `package-release-defaults/tasks.md`, `examples/realworld/package-release-loop`, `tools/sgpm/tests/integration.rs`, `docs/sgpm-quickstart.md`, `docs/internal-release.md` | `package-release-defaults` |

## Upstream prerequisites

- Archived package graph and tooling work remains the baseline:
  `sgpm-alias-multiversion`, `ecosystem-toolchain-maturity`, and
  `toolchain-internal-ux`.
- `package-release-defaults` must not duplicate resolver semantics already
  archived into `sgpm-package-graph`; it owns deterministic publish/release
  evidence on top of those semantics.
- `async-default-followups` coordinates with archived async/runtime children but
  owns the remaining default-readiness claims.

## Current front-five child table

| Block | Change | Delta ownership | Status |
| --- | --- | --- | --- |
| 1 | `async-default-followups` | `async-default-followups`, async/runtime support matrix | Open by design; same-thread user Future and cleanup wrappers are supported with proof, while all-host fd readiness, broader user-future diagnostics, and cancellation boundaries remain explicitly Deferred |
| 2 | `stdlib-https-tls` | `stdlib-mainstream-usability` | Open; POSIX/reference-host TLS evidence pending |
| 3 | `stdlib-default-followups` | `stdlib-mainstream-usability` | Implemented locally; compression is `Supported subset` with resource limits, deterministic stored-gzip behavior, and a realworld fixture |
| 4 | `language-default-polish` | `language-default-polish` | Implemented locally; pinned cfg predicates, FFI negative diagnostics, async payload-enum deferral, and match/try/LSP parity are covered |
| 5 | `package-release-defaults` | `sgpm-package-graph`, `tooling-mainstream-ecosystem` | Implemented locally; deterministic publish, registry diagnostics/cache, release fixture, and release smoke are validated on Windows, with Ubuntu/reference-host deterministic evidence pending. This workspace lacks an installed WSL distribution and Docker, so the Ubuntu matrix must be proven by CI or a separate reference host. |

## Support matrix archive blockers

- Async rows must reflect `async-default-followups`: user-defined Future,
  all-host owned-fd readiness, task cancellation boundaries, and select loser
  cancellation are supported only with proof or remain Deferred.
- TLS/HTTPS rows must reflect `stdlib-https-tls`: POSIX/reference-host success
  must be proven or the row remains `Platform-specific` with an evidenced skip.
- Compression is promoted to `Supported subset` after `stdlib-default-followups`
  landed real gzip-compatible APIs, resource-limit tests, and a realworld
  fixture.
- Package/release defaults cite `examples/realworld/package-release-loop` and
  `tools/sgpm/tests/integration.rs::realworld_package_release_loop_covers_publish_defaults`;
  final cross-host archive proof still requires Ubuntu/reference-host
  deterministic evidence.
- Language polish changes that alter user-visible support must update compiler,
  JSON diagnostic, and `sglsp` proof paths before umbrella archive.
