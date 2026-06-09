## Why

Sengoo tooling is internally usable but not mainstream-thick: cross-compilation is
undocumented, registry workflows are MVP-shaped, and LSP depth lags rust-analyzer /
gopls.

## What Changes

- Cross-compile target triple workflow for `sgc build` with documented host/sysroot policy.
- Supersede `sgpm-alias-multiversion` as the single active package graph owner,
  copying forward alias, multi-version, lockfile v2, and metadata edge rules.
- Registry metadata: yanked versions, feature flags in `sgpm metadata --format json`.
- LSP: cross-package go-to-definition for path/git deps, deprecated/cfg already done.
- Observability: `sgc build --timings` JSON export for CI dashboards.
- Public registry publish checklist (not population).

## Capabilities

### New Capabilities

- None.

### Modified Capabilities

- `tooling-mainstream-ecosystem`: cross-compile, timings JSON, LSP depth.
- `sgpm-package-graph`: canonical owner for dependency aliases, multi-version
  package identities, lockfile v2, metadata dependency edges, and yanked +
  features metadata.

## Impact

- `tools/sgc/`, `tools/sglsp/`, `tools/sgpm/`, `docs/`
- Parent umbrella: `mainstream-production-readiness` Block 4

## Prerequisites

- `sgpm-alias-multiversion` is explicitly superseded here by copying forward its
  canonical package-graph deltas. Registry metadata behavior still requires the
  corresponding `tools/sgpm` implementation and tests before this change can archive.
