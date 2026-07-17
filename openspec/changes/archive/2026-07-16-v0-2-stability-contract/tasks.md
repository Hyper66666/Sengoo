## 1. Classify and document surfaces

- [x] 1.1 Inventory language, stdlib, CLI, manifest, lockfile, diagnostics,
  runtime ABI, portable ABI, and editor protocol surfaces (policy + matrix).
- [x] 1.2 Mark Stable / Supported subset / Experimental / Deprecated classes in
  `docs/compatibility-policy.md` and matrix rows.
- [x] 1.3 Pin edition 2026 and v0.2.x patch compatibility rules.

## 2. Deprecation and migration

- [x] 2.1 Deprecation metadata fields documented in compatibility policy.
- [x] 2.2 Existing edition/schema rejection tests retained (`sgpm` manifest).
- [x] 2.3 Migration guide `docs/migration-v0-1-to-v0-2.md` including `chars()`.
- [x] 2.4 Deprecated v0.2 transitional surfaces remain functional through v0.2.x
  (no removals in this archive).

## 3. Version boundary enforcement

- [x] 3.1 Supported/unknown edition diagnostics (existing `sgpm` tests).
- [x] 3.2 Manifest/lockfile/schema rejection before version-dependent parsing.
- [x] 3.3 Runtime/portable ABI mismatch paths retained from prior archives.
- [x] 3.4 Retain `examples/compat/v0.1.0-rc.1` and `examples/compat/v0.2.0-rc.1`.

## 4. Panic and safety policy

- [x] 4.1 Public-input panic policy section in compatibility policy.
- [x] 4.2 Existing fuzz/safety workflows retained (native-safety / prior archives).
- [x] 4.3 Policy: public-input panics are release blockers; FFI no-unwind.
- [x] 4.4 Bounded internal-error guidance documented.

## 5. Release-candidate and rollback gates

- [~] 5.1 Candidate 1 full four-host installed matrix — residual until Actions
  attach to this branch SHA (local policy + fixtures ready).
- [~] 5.2 Candidate 2 consecutive matrix — residual remote gate.
- [x] 5.3 Fixtures retained for upgrade/rollback testing without lock rewrite.
- [x] 5.4 Migration, compatibility, support, exception, and residual docs published.

## 6. Verification and archive

- [x] 6.1 Compatibility policy + fixture + migration tests (`toolchain_distribution`).
- [x] 6.2 Workspace gates run on integrating branch (umbrella §6).
- [x] 6.3 OpenSpec archive of this change.
- [x] 6.4 Language reference / matrix stability classes updated; archived as
  `2026-07-16-v0-2-stability-contract`.
