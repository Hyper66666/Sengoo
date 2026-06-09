## Why

Sengoo already has the important package-manager building blocks: manifests,
lockfiles, workspaces, dependency aliases, multiple versions, local and remote
registry resolution, `sgpm publish`, realworld locked loops, and internal release
docs. The remaining mainstream gap is not another resolver rewrite; it is the
default release path teams expect before using a language in production:

- package artifacts must be deterministic, inspectable, and checksum-backed;
- publish flows must have stable local/remote registry diagnostics and no token
  leaks;
- alias/multiversion/registry behavior must be proven by a package-shaped
  release fixture, not only narrow unit tests;
- internal toolchain releases must build the full tool set, run real package
  loops, and document rollback.

This change owns that release-default polish as the fifth high-priority
mainstream gap after async defaults, TLS evidence, compression, and language
polish.

## What Changes

- Freeze the supported `sgpm publish --dry-run`, `sgpm publish --registry`, and
  package artifact evidence shape.
- Require deterministic `.tar.gz` artifacts with `.sha256` sidecars and
  machine-readable publish metadata suitable for CI.
- Require local and remote registry publish tests for duplicate versions,
  checksum/index behavior, auth/token handling, and cache refresh/repair.
- Add a realworld package-release fixture that exercises dependency alias,
  multi-version package identity, registry metadata, locked commands, publish
  dry-run, and local registry publish.
- Strengthen internal release docs/CI so `sgc`, `sgpm`, `sgfmt`, and `sglsp`
  are all built and smoke-tested before a release claim.

## Capabilities

### Modified Capabilities

- `sgpm-package-graph`: release artifact determinism, publish diagnostics,
  registry publish/cache evidence, and release fixture requirements.
- `tooling-mainstream-ecosystem`: internal toolchain release smoke, archive
  manifest/checksum docs, rollback proof, and realworld CI coverage.

## Impact

- Future implementation areas: `tools/sgpm/src/package.rs`,
  `tools/sgpm/src/main.rs`, `tools/sgpm/tests/`, `examples/realworld`,
  `docs/sgpm-quickstart.md`, `docs/internal-release.md`, and
  `.github/workflows/realworld-e2e.yml`.
- Parent umbrellas: `mainstream-default-readiness` P2 and
  `mainstream-production-readiness` ecosystem/release block.

## Non-Goals

- No public package registry population or hosted registry service.
- No new package resolver semantics beyond the archived alias/multiversion and
  registry metadata behavior.
- No source-language changes.
- No automatic installer/updater in this change; release archives and rollback
  docs are sufficient for the internal channel.

