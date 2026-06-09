## Why

Internal monorepos need dependency aliases and multiple versions of the same
package name. `sgpm` currently rejects both. Pillar 3 of
`six-pillar-gap-closure` owns resolver/lockfile changes separately from stdlib
and compiler work.

## Superseded

This change no longer owns the canonical active `sgpm-package-graph` delta.
`openspec/changes/ecosystem-toolchain-maturity` copies forward the alias,
multi-version, lockfile v2, and metadata edge requirements from this change and
adds the remaining registry metadata requirements. Keep this change active only
as historical implementation evidence until the lead archives or removes it.

## What Changes

- Add `package = "actual_name"` on dependency tables for renamed keys.
- Bump lockfile schema to `version = 2` with package identity
  `(name, version, source)` and edge-level aliases.
- Retain compatible `version = 1` reads; deterministic `sgpm update` migration.
- Extend `sgpm metadata --format json` with alias mapping.
- Document internal monorepo/registry workflow.

## Capabilities

### New Capabilities

- None. Superseded by `ecosystem-toolchain-maturity`.

### Modified Capabilities

- `sgpm-package-graph`: superseded historical draft for renamed dependencies,
  multi-version resolution, lockfile v2, and metadata dependency edges.

## Impact

- `tools/sgpm/src/manifest.rs`, `resolver.rs`, `lockfile.rs`, integration tests
- `docs/sgpm-quickstart.md`, workspace examples
- Parent umbrella: `six-pillar-gap-closure` Pillar 3

## Parent

- Umbrella: `six-pillar-gap-closure`
- Resolver rules: `openspec/changes/six-pillar-gap-closure/design.md` §Pillar 3
