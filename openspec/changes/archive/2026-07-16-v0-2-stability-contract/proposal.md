## Why

Sengoo already versions its runtime/portable ABI, manifests, lockfiles,
diagnostic/test schemas, releases, and compatibility fixtures. To become a
dependable language, those mechanisms must form one user-facing contract:
supported v0.2 programs must survive patch upgrades, deprecated behavior must
have a migration window, malformed public input must not crash tools, and a
release must be proven by repeated installed-artifact gates rather than one
successful CI run.

## What Changes

- Define Stable, Supported subset, Experimental, and Deprecated surface classes.
- Pin v0.2.x source/tool/stdlib compatibility and edition behavior.
- Require deprecation diagnostics, migration guidance, and removal windows.
- Enforce version rejection across language edition, manifest, lockfile,
  diagnostic/test JSON, MIR, runtime ABI, and portable ABI boundaries.
- Convert unclassified public-input panics into release blockers with retained
  regressions and bounded internal-error reporting.
- Require two consecutive v0.2 release-candidate matrices before v0.2.0.

## Capabilities

### Modified Capabilities

- `production-hardening`: add v0.2 compatibility, panic, repeated-RC, and
  rollback gates.
- `language-reference`: require per-surface stability class, edition behavior,
  deprecation window, and migration record.

## Impact

- Compatibility docs, reference, compiler/package/tool version parsing,
  diagnostics, retained fixtures, fuzz/safety workflows, release workflows, and
  migration notes.
- Stable v0.2.x surfaces gain stricter change control; experimental surfaces do
  not gain compatibility guarantees.

## Non-Goals

- Declaring Sengoo 1.0 or freezing all ABI forever.
- Guaranteeing compatibility for unsound behavior, security defects, or surfaces
  explicitly marked Experimental.
- Catching hardware faults or foreign-library process termination.
- Requiring zero internal assertions; only public input must fail safely.
