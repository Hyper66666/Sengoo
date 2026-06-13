## ADDED Requirements

### Requirement: The P0–P2 language-maturity program SHALL be delivered through independently archived child changes

The umbrella SHALL coordinate one required child change per pillar so that each
canonical capability delta stays independently reviewable, revertible, and
archivable. The umbrella SHALL NOT substitute its aggregate requirements for a
child capability delta.

#### Scenario: A pillar begins implementation

- **WHEN** implementation work begins for one of the eleven pillars
- **THEN** the pillar has the child change id listed in `proposal.md`
- **AND** that child change owns its capability delta, design decisions, tasks,
  tests, and archive gate
- **AND** any active upstream change owning the same capability is archived first
  or recorded as an explicit blocker

#### Scenario: The umbrella is proposed for archive

- **WHEN** `language-maturity-roadmap` is proposed for archive
- **THEN** all eleven required child changes have already passed `--strict`
  validation and been archived
- **AND** no accepted-risk or deferred matrix row stands in for an unimplemented
  pillar

### Requirement: The program SHALL standardize one coherent default memory model

The program SHALL adopt move-based ownership with compiler-inserted `Drop` as
the single default memory model and SHALL record any deviation from that
decision in `design.md` before code lands.

#### Scenario: Idiomatic code releases resources without manual calls

- **WHEN** the P0 gate is evaluated
- **THEN** at least one realworld fixture compiles and runs with zero manual
  `.free()`, `.drop()`, or `.close()` calls
- **AND** the released resources include a heap container, an owned string, and a
  runtime handle (file, buffer, or json document)

### Requirement: Transitions SHALL be additive and source-compatible

Each child change SHALL add safe APIs alongside existing Buffer/handle APIs and
SHALL keep existing public names source-compatible for the duration of this
program.

#### Scenario: Existing example still compiles after a child lands

- **WHEN** a child change introduces a new safe API for an existing capability
- **THEN** the previously committed examples and realworld fixtures that used the
  old handle/Buffer API still compile and run unchanged
- **AND** any deprecation is proposed only in a later, separate change

### Requirement: The program SHALL update the public support matrix on closure

On umbrella archive, the program SHALL move the affected capabilities from
"subset"/"deferred" to "Supported" with proof, so the support matrix stays the
single source of truth.

#### Scenario: Support matrix reflects delivered capabilities

- **WHEN** the umbrella is archived
- **THEN** `examples/realworld/SUPPORT_MATRIX.md` lists memory safety, generics,
  strings, numerics, and concurrency rows as Supported with linked proof
  examples/tests
- **AND** `README.md` no longer describes these as MVP-only limitations
