# production-hardening Specification

## Purpose
Define the retained safety, compatibility, performance, and installed-release
evidence required before Sengoo can claim its native toolchain is production
ready on a supported host.
## Requirements
### Requirement: Public input boundaries SHALL be fuzzed and regression-retained

The compiler, package manager, and runtime parsing boundaries SHALL have bounded
fuzz targets, and fixed crashes SHALL produce retained regression evidence.

#### Scenario: A fuzzer finds a compiler crash

- **WHEN** malformed source causes a panic, unbounded allocation, or invalid MIR
- **THEN** the issue is fixed
- **AND** a minimized corpus entry or deterministic regression test is retained
- **AND** the bounded per-commit fuzz gate exercises that input

### Requirement: Native and FFI boundaries SHALL pass safety and leak gates

Supported native hosts SHALL run sanitizer or equivalent memory-safety tests,
resource leak checks, and invalid pointer/length/handle boundary tests.

#### Scenario: A native handle is stale or released twice

- **WHEN** runtime or FFI code receives a stale generation handle or duplicate
  release
- **THEN** it returns the documented stable status without use-after-free,
  double free, panic across FFI, or leaked ownership

### Requirement: Compatibility and runtime ABI SHALL be versioned

The project SHALL publish source/edition compatibility, deprecation, manifest,
lockfile, diagnostic schema, and runtime ABI policies and SHALL diagnose
unsupported version combinations.

#### Scenario: A package uses an incompatible runtime ABI

- **WHEN** a compiled artifact or package requires an unsupported runtime ABI
- **THEN** build or launch fails before unsafe execution
- **AND** the diagnostic reports the required and available ABI versions

### Requirement: Performance and resource budgets SHALL be enforced

Committed benchmark scenarios SHALL have reviewed wall-time, peak-RSS, artifact-
size, startup, and representative runtime budgets.

#### Scenario: A compiler change exceeds a budget

- **WHEN** a reference scenario exceeds its allowed regression threshold or
  absolute resource ceiling
- **THEN** CI fails
- **AND** changing the budget requires recorded benchmark evidence and review

### Requirement: Release readiness SHALL be proven with installed artifacts

Realworld and official-package release gates SHALL use installed release
artifacts outside the source checkout on every supported host.

#### Scenario: A release candidate is evaluated

- **WHEN** a release candidate reaches the production gate
- **THEN** each supported host installs and verifies the artifact
- **AND** locked package check/test/doc/build/run succeeds without workspace path
  leakage
- **AND** skipped jobs outside the support matrix are documented rather than
  counted as success

### Requirement: Stable v0.2.x surfaces SHALL preserve patch compatibility

Stable surfaces SHALL remain compatible throughout v0.2.x after v0.2.0.
Source language, stdlib, CLI, manifest, lockfile, diagnostic/protocol, and ABI
surfaces classified Stable at v0.2.0 stay compatible except for documented
security or unsoundness corrections with diagnostics and migration guidance.

#### Scenario: Stable package upgrades to a later patch

- **WHEN** a retained v0.2.0 package is checked, tested, built, and run with a
  later v0.2.x installed toolchain
- **THEN** it succeeds without source or lockfile rewriting
- **AND** observable Stable behavior remains within the documented contract

#### Scenario: Unsound accepted behavior must be rejected

- **WHEN** preserving compatibility would retain a security or soundness defect
- **THEN** the correction includes a published exception notice, stable
  diagnostic, affected-version range, and migration path
- **AND** a retained regression proves safe rejection

### Requirement: Deprecation SHALL precede Stable surface removal

A Stable surface SHALL remain functional for the current v0.2.x line after
deprecation and SHALL emit a stable warning naming its replacement and earliest
removal version. Removal requires a later accepted minor/edition change.

#### Scenario: Deprecated API is compiled

- **WHEN** a program uses a deprecated Stable API during v0.2.x
- **THEN** it still compiles/runs within its prior contract
- **AND** text, JSON, and LSP output carry the same deprecation code,
  replacement, and removal horizon

### Requirement: Public input SHALL not cause unclassified tool panics

Public tooling SHALL return bounded stable diagnostics for malformed or hostile
input and SHALL NOT expose an uncaught implementation panic as normal failure
behavior. Covered inputs include source, manifests, lockfiles, package archives,
protocol payloads, runtime handles, and portable artifacts.

#### Scenario: Fuzzing finds a public-input panic

- **WHEN** a public input reaches an uncaught panic, unbounded allocation, FFI
  unwind, or process abort
- **THEN** release gates fail
- **AND** the fix retains a minimized corpus entry or deterministic regression

#### Scenario: Unexpected internal invariant fails

- **WHEN** a CLI can safely catch an unexpected internal failure
- **THEN** it emits a bounded internal-error envelope with tool version and
  backtrace instructions
- **AND** does not misclassify the failure as a user syntax/type error

### Requirement: Versioned boundaries SHALL reject unknown versions before interpretation

Versioned consumers SHALL check edition, manifest, lockfile, registry,
diagnostic/test JSON, editor protocol, MIR semantic ABI, runtime ABI, and
portable ABI versions before version-dependent fields are consumed or code is
executed.

#### Scenario: Explicit version is newer than supported

- **WHEN** a versioned artifact declares an unknown explicit version
- **THEN** the consumer rejects it with required/available version information
- **AND** does not guess, downgrade, rewrite, link, or execute it

### Requirement: v0.2.0 SHALL require two consecutive release-candidate gates

Two candidate commits SHALL each pass installed-artifact, compatibility, safety,
performance, realworld, and strict OpenSpec matrices on every supported host
before v0.2.0 is published.

#### Scenario: Stable behavior changes after one passing candidate

- **WHEN** a P0/P1 fix changes a Stable contract after candidate 1 passes
- **THEN** the consecutive-candidate count resets
- **AND** the changed candidate becomes the new candidate 1 baseline

### Requirement: Release rollback SHALL be executable and non-destructive

The previous published toolchain SHALL remain installable and its rollback smoke
SHALL verify retained packages without silently rewriting manifests or lockfiles.

#### Scenario: Maintainer rolls back a failed candidate

- **WHEN** the previous archive is reinstalled and its checksum verified
- **THEN** retained compatible packages pass their locked loop
- **AND** newer incompatible artifacts fail with actionable version diagnostics
