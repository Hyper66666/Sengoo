# Baseline Inventory (mainstream-production-readiness)

## Block status (June 2026)

`mainstream-production-readiness` now tracks the current front-five blockers.
Compile scale is retained as closed historical evidence and must not reopen as a
competing blocker unless new measurements regress below the accepted gate.

| Block | Gap | Current evidence | Child change |
| --- | --- | --- | --- |
| 0 Compile scale evidence | Closed/superseded 1000k RSS/frontend-share gate | `bench/results/1780946346830-advanced-pipeline.json`, `mainstream-default-readiness/INVENTORY.md` | `compile-scale-production-gate` historical evidence |
| 1 Async defaults | User `Future::poll`, all-host owned-fd readiness, cancellation boundaries, public cleanup-wrapper lowering | `docs/runtime-async-semantics.md`, `examples/realworld/SUPPORT_MATRIX.md`, `openspec/specs/async-default-followups/spec.md` | canonical `async-default-followups` spec |
| 2 HTTPS/TLS evidence | POSIX/reference-host trusted success, hostname mismatch, and HTTPS runtime roundtrip proof | `openspec/specs/stdlib-mainstream-usability/spec.md`, `examples/realworld/http-client-status`, `six-pillar-gap-closure` CI gap | canonical stdlib spec plus active umbrella evidence gap |
| 3 Stdlib compression | Real gzip-compatible `std::compress` APIs, resource limits, deterministic gzip behavior, realworld fixture | `openspec/specs/stdlib-mainstream-usability/spec.md`, `tools/stdlib/README.md`, `examples/realworld/SUPPORT_MATRIX.md` | canonical stdlib spec |
| 4 Language polish | Pinned cfg predicates, FFI rejection parity, payload-enum async frame decision, match/try diagnostic parity | `openspec/specs/language-default-polish/spec.md`, compiler/sglsp diagnostics tests | canonical `language-default-polish` spec |
| 5 Package/release defaults | Deterministic publish artifacts, local/remote registry publish evidence, release fixture, release smoke/rollback docs | `openspec/specs/sgpm-package-graph/spec.md`, `openspec/specs/tooling-mainstream-ecosystem/spec.md`, `examples/realworld/package-release-loop`, `tools/sgpm/tests/integration.rs`, `docs/sgpm-quickstart.md`, `docs/internal-release.md` | canonical package/tooling specs |

## Upstream prerequisites

- Archived package graph and tooling work remains the baseline:
  `sgpm-alias-multiversion`, `ecosystem-toolchain-maturity`, and
  `toolchain-internal-ux`.
- Future package-release changes must not duplicate resolver semantics already
  archived into `sgpm-package-graph`; deterministic publish/release evidence is
  now promoted into canonical package/tooling specs.
- Future async follow-ups coordinate with archived async/runtime children and
  the promoted `async-default-followups` canonical spec.

## Current front-five child table

| Block | Change | Delta ownership | Status |
| --- | --- | --- | --- |
| 1 | promoted `async-default-followups` spec | `async-default-followups`, async/runtime support matrix | Same-thread user Future and cleanup wrappers are supported with proof, while all-host fd readiness, broader user-future diagnostics, and cancellation boundaries remain explicitly Deferred |
| 2 | promoted stdlib TLS requirements | `stdlib-mainstream-usability` | POSIX/reference-host TLS evidence still needs CI/reference-host proof and is tracked by `six-pillar-gap-closure` |
| 3 | promoted stdlib compression requirements | `stdlib-mainstream-usability` | Compression is `Supported subset` with resource limits, deterministic stored-gzip behavior, and a realworld fixture |
| 4 | promoted `language-default-polish` spec | `language-default-polish` | Pinned cfg predicates, FFI negative diagnostics, async payload-enum deferral, and match/try/LSP parity are covered |
| 5 | promoted package/tooling release requirements | `sgpm-package-graph`, `tooling-mainstream-ecosystem` | Deterministic publish, registry diagnostics/cache, release fixture, and release smoke are validated on Windows; Ubuntu/reference-host deterministic evidence should come from CI/reference host |

## Support matrix archive blockers

- Async rows must reflect the canonical `async-default-followups` spec: user-defined Future,
  all-host owned-fd readiness, task cancellation boundaries, and select loser
  cancellation are supported only with proof or remain Deferred.
- TLS/HTTPS rows must reflect the canonical stdlib TLS requirements: POSIX/reference-host success
  must be proven or the row remains `Platform-specific` with an evidenced skip.
- Compression is promoted to `Supported subset` after archived stdlib default follow-up work
  landed real gzip-compatible APIs, resource-limit tests, and a realworld
  fixture.
- Package/release defaults cite `examples/realworld/package-release-loop` and
  `tools/sgpm/tests/integration.rs::realworld_package_release_loop_covers_publish_defaults`;
  final cross-host archive proof still requires Ubuntu/reference-host
  deterministic evidence.
- Language polish changes that alter user-visible support must update compiler,
  JSON diagnostic, and `sglsp` proof paths before umbrella archive.
