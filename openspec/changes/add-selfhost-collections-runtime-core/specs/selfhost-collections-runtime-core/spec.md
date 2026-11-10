## ADDED Requirements

### Requirement: An explicit bounded SG registry SHALL own handle algorithms

The experimental caller-context profile SHALL implement slot allocation, identity,
generation, kind checks, lookup and release in SG. The context SHALL not replace
the existing process-global reference registry or confer provider authority.

#### Scenario: Wrong context or stale handle
- **WHEN** an operation receives another admitted caller identity, a closed handle,
  a wrong kind, or a retired generation
- **THEN** it rejects the handle without accessing another slot's payload
- **AND** generation exhaustion does not resurrect a stale handle.

#### Scenario: Context lifetime and unsafe identity responsibility
- **WHEN** an initialized context is validated, reset empty with a different caller
  identity, disposed, or disposed twice
- **THEN** validation is read-only, reset/disposal follow the declared lifecycle,
  and the second disposal fails
- **AND** the contract explicitly excludes reuse of the same identity across
  lifetimes/contexts as a caller error, not a safety guarantee.

#### Scenario: Bad extent
- **WHEN** init, validation, vector, iterator, reset, or disposal receives an invalid
  context extent or NULL context
- **THEN** it returns the documented error before any state or payload access.

### Requirement: RawVec storage and ownership algorithms SHALL be SG

SG SHALL implement descriptor-driven growth, element movement, replacement,
removal, clear, drop, and iteration for the admitted bounded profile. Memory and
callback C adapters SHALL contain no registry or collection algorithm.

#### Scenario: Arbitrary element shape and actual owning values
- **WHEN** i64, bool, packed three-byte values, aligned 16-byte owners, and
  eight-byte owners grow, mutate, iterate, transfer, clear, and drop
- **THEN** values, callback order/count, status, and ownership agree with the real
  reference within the documented profile
- **AND** size, flags, or callback non-nullness never classify a value as Copy.

#### Scenario: Failure prefix
- **WHEN** payload allocation fails, context slots are exhausted, indices are
  invalid, or output is not found
- **THEN** prior live elements remain valid, input consumption follows the declared
  status contract, outputs are unchanged when not found, and resources are exact.

#### Scenario: Iterator resource correction
- **WHEN** iterator construction cannot acquire a registry slot
- **THEN** the SG implementation returns failure with no allocated iterator leak
- **AND** the independent verifier records the real old-reference failure behavior
  rather than claiming that resource defect is parity.

### Requirement: Qualification SHALL bind actual tools and deployment

The gate SHALL bind consumed source snapshots, actual native emitter/formatter,
historically qualified SG linker branch, C headers/platform imports, reference
objects, artifacts and command receipts, with actual revision/host and guards.

#### Scenario: Pure SG object and linker execution
- **WHEN** two fresh directories emit the registry/vector/smoke objects and use the
  qualified SG linker with the exact declared platform closure
- **THEN** both empty-PATH products execute successfully and all corresponding SG,
  adapted C and PE bytes agree
- **AND** the report does not imply global SDK, compiler self-build, or M8 completion.
