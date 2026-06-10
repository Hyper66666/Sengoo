# mainstream-default-readiness Specification

## Purpose
TBD - created by archiving change mainstream-default-readiness. Update Purpose after archive.
## Requirements
### Requirement: Mainstream default readiness SHALL be priority ordered

Sengoo SHALL track mainstream-default readiness through a priority-ordered
program: compile scale first, async/runtime maturity second, ecosystem/release
defaults third, stdlib thickness fourth, and language polish fifth.

#### Scenario: Higher-priority gates block archive

- **WHEN** a reviewer evaluates this umbrella for archive
- **THEN** unmet higher-priority gates block archive even if lower-priority
  features have landed
- **AND** lower-priority work may remain active only when its ownership and
  evidence do not obscure the higher-priority gap

#### Scenario: Existing child changes are reused

- **WHEN** a remaining gap is already owned by an active child change
- **THEN** this umbrella SHALL cite that child change instead of duplicating
  requirements
- **AND** any supersession SHALL name the old change and copied-forward evidence

### Requirement: P0 compile scale SHALL prove large-repo readiness

Sengoo SHALL NOT claim mainstream-default readiness until the compile-scale gate
has required reference-host evidence for 100k and 1000k workloads. A 2500k
workload is optional/report-only stretch evidence when runnable.

#### Scenario: Compile-scale archive evidence is present

- **WHEN** P0 is considered complete
- **THEN** benchmark evidence includes median peak RSS, frontend share, workload
  size, host profile, and comparison baseline
- **AND** native/runtime cache identity tests still pass after optimizations

### Requirement: P1 async/runtime maturity SHALL document supported defaults

Async and runtime behavior SHALL distinguish default cooperative behavior,
opt-in concurrent behavior, async IO readiness, user futures, cancellation, and
unsupported shapes.

#### Scenario: Async docs match implementation

- **WHEN** async runtime child changes archive
- **THEN** `docs/runtime-async-semantics.md` and
  `examples/realworld/SUPPORT_MATRIX.md` agree on supported and deferred rows
- **AND** realworld examples use public stdlib APIs rather than compiler-only
  fixtures

### Requirement: P2 ecosystem/release defaults SHALL make adoption routine

Package, registry, cross-compile, IDE, and release workflows SHALL be documented
and tested enough for a new internal project to adopt Sengoo without hidden repo
knowledge.

#### Scenario: Package graph ownership is clear

- **WHEN** registry metadata, package aliases, multi-version graphs, or lockfile
  schema behavior changes
- **THEN** exactly one active change owns the canonical package graph delta
- **AND** locked commands do not silently reinterpret old lockfiles

#### Scenario: Release workflow is evidenced

- **WHEN** a release channel is claimed
- **THEN** versioned binaries, smoke matrix, rollback instructions, and supported
  host policy are documented
- **AND** at least one real toolchain e2e test exercises the release workflow or
  its local equivalent

### Requirement: P3 stdlib thickness SHALL be demand driven

New stdlib modules and expansions SHALL be selected from realworld demand and
SHALL include resource limits, stable statuses, examples, and support matrix
rows.

#### Scenario: A new stdlib feature is proposed

- **WHEN** a new stdlib module or major feature is proposed
- **THEN** its OpenSpec defines API shape, ownership/lifecycle, resource limits,
  platform behavior, and stable failure status
- **AND** at least one test or realworld fixture proves the supported path

### Requirement: P4 language polish SHALL prefer additive relaxations

Language surface polish SHALL remove phase-only restrictions through additive
changes first and SHALL require migration policy for breaking cleanup.

#### Scenario: A phase-only restriction is relaxed

- **WHEN** parser, typeck, lowering, async frame, FFI, attribute, match/try, or
  diagnostic behavior is relaxed
- **THEN** compiler tests cover the accepted shape and negative tests cover
  rejected unsound shapes
- **AND** `sglsp` diagnostics and quick fixes are updated when user-facing
  diagnostics change

### Requirement: Support matrices SHALL remain the user-facing truth

Support matrices SHALL be updated whenever a mainstream-default claim changes.
Rows may use `Accepted risk` only for implemented behavior that has internal
runtime proof but lacks enough realworld or reference-host evidence to be
claimed as supported.

#### Scenario: A supported subset is claimed

- **WHEN** docs or proposals claim a capability is supported or supported subset
- **THEN** the relevant support matrix row cites proof tests/examples and stable
  diagnostics
- **AND** unsupported or platform-specific behavior is not hidden behind success
  examples alone
