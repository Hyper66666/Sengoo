## 1. Preparation

- [x] 1.1 Run `openspec validate sgpm-alias-multiversion --strict`.
- [x] 1.2 Run `openspec validate --all --strict`.
- [x] 1.3 Superseded by `ecosystem-toolchain-maturity`; copied-forward canonical
  package graph deltas now live there.

## 2. Implementation

- [x] 2.1 Add manifest `package` field on dependencies.
- [x] 2.2 Implement lockfile v2 schema and source canonicalization.
- [x] 2.3 Implement v1 read, deterministic v2 migration, and locked-command failure paths.
- [x] 2.4 Extend `sgpm metadata --format json`.

## 3. Verification

- [x] 3.1 `cargo test -p sgpm alias`
- [x] 3.2 `cargo test -p sgpm multiversion`
- [x] 3.3 `cargo test -p sgpm lockfile`
- [x] 3.4 Update `docs/sgpm-quickstart.md`

## Archive Gate

- [x] `openspec validate sgpm-alias-multiversion --strict` passes.
- [x] `openspec validate --all --strict` passes.
- [x] Lockfile golden tests cover v1 read, v2 write, alias edges, and incompatible-graph diagnostics.
- [x] Canonical active ownership transferred to
  `openspec/changes/ecosystem-toolchain-maturity`.
