## ADDED Requirements

### Requirement: The maturity program SHALL use independently archived execution lanes

The program SHALL assign each capability to one child change with its own
design, tasks, tests, and archive gate. `language-maturity-roadmap` SHALL be the
only active coordinator for this program.

#### Scenario: A lane starts implementation

- **WHEN** implementation begins for a roadmap capability
- **THEN** the owning child change is named in `proposal.md`
- **AND** dependencies and shared-file ownership are recorded
- **AND** overlapping active changes are archived, superseded, or recorded as
  blockers before code is changed

### Requirement: Mainline reconciliation SHALL precede new capability work

The program SHALL preserve and integrate the current development state into a
clean, reviewable mainline before later phases claim completion.

#### Scenario: Phase 1 implementation is proposed

- **WHEN** numeric, collections, or debugger implementation begins under this
  roadmap
- **THEN** `mainline-release-baseline` has passed its integration and truth-
  reconciliation gates
- **AND** verification is run from the integrated branch rather than an obsolete
  divergent worktree

### Requirement: Backend capability tiers SHALL define support claims

Every backend SHALL be classified as production, experimental, or deferred.
Only production backends define release-blocking language behavior.

#### Scenario: An experimental backend receives an unsupported program

- **WHEN** the program uses a capability outside the backend's documented
  subset
- **THEN** the backend rejects it explicitly
- **AND** the rejection is not treated as a language-semantics failure
- **AND** public documentation does not imply production parity

### Requirement: The mainstream default release SHALL close the complete user path

The first mainstream-default milestone SHALL include generic owning collections,
documented numeric semantics, source-level debugging, registry-backed locked
dependencies, installable releases, safe generic concurrency, and production
hardening evidence.

#### Scenario: An external user exercises the default path

- **WHEN** a user starts from a clean supported host
- **THEN** they can install Sengoo, create or fetch a package, resolve locked
  dependencies, build, test, debug, and run it without a source checkout
- **AND** the package can use generic collections and automatic resource release
- **AND** failures have stable diagnostics rather than silent fallback

### Requirement: Public support claims SHALL be evidence-tiered

The support matrix SHALL distinguish unit, native integration, realworld, and
release-host evidence, and SHALL keep platform-limited capabilities explicitly
platform-specific.

#### Scenario: A capability is promoted to Supported

- **WHEN** a roadmap capability is changed from subset/deferred/platform-
  specific to Supported
- **THEN** the support-matrix row links evidence matching the scope of the claim
- **AND** skipped or unavailable host tests do not count as passing evidence

### Requirement: Alternative backends SHALL wait for a stable ABI checkpoint

WASM and bytecode implementation SHALL begin only after the native MIR/runtime
ABI is versioned and the default-library, release, concurrency, and production-
hardening entry gates pass.

#### Scenario: Alternative backend work is scheduled

- **WHEN** WASM or bytecode implementation is proposed
- **THEN** the stable-ABI entry review is recorded
- **AND** WASM and bytecode have separate owner changes and conformance gates
- **AND** neither backend blocks the earlier mainstream-default release

### Requirement: Roadmap archive gates SHALL distinguish native mainstream from post-v1 portable work

The language-maturity roadmap SHALL treat Phases 0-4 native mainstream completion
as independent from Phase 6+ portable backend completion. Full umbrella archive
MAY remain open while experimental WASM work continues, and MUST NOT re-open
archived native phase claims solely because post-v1 tasks are incomplete.

#### Scenario: Native mainstream is complete but WASM is still experimental

- **WHEN** Phases 0-4 are archived with release-host evidence and experimental
  WASM remains an open child change
- **THEN** native mainstream-default support claims may stay closed
- **AND** the umbrella change remains unarchived until post-v1 tasks close or
  are explicitly split to a successor program change
