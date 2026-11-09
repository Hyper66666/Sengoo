## ADDED Requirements

### Requirement: Canonical iterator origins SHALL require verified producers

Origin analysis SHALL consume actual typed module facts and independently
caller-pinned physical collection bytes, declarations and prototypes.

#### Scenario: A genuine Vec creates an iterator
- **WHEN** an admitted Vec constructor and an actual canonical same-element iterator factory produce the value
- **THEN** a bounded owned origin record SHALL identify the concrete element and backing binding
- **AND** arbitrary source names, matching layout or descriptor callback presence SHALL NOT mint origin

#### Scenario: A record is reconstructed or has no storage
- **WHEN** an iterator record uses an unknown handle or a handle extracted from a different element owner
- **THEN** its ordinary type behavior MAY remain valid but new borrow origin eligibility SHALL be absent
- **AND** literal zero placeholders SHALL remain inert rather than live or Copy origins

### Requirement: Iterator parameter origins SHALL remain conditional

A nominal parameter type SHALL NOT initialize unconditional producer authority.

#### Scenario: A parameter iterator or vector is forwarded
- **WHEN** a function uses or returns a parameter's canonical origin
- **THEN** the result SHALL retain a conditional origin obligation tied to the actual parameter and element
- **AND** a caller supplying no proven origin SHALL NOT turn it into a concrete origin

#### Scenario: A local backing vector escapes
- **WHEN** an iterator return depends on a local backing owner that does not escape alive
- **THEN** the origin layer SHALL withhold return authority and retain an explicit diagnostic

### Requirement: Origin validation SHALL preserve typed inputs and partial boundaries

Origin validation SHALL be bounded and read-only with respect to input type
arenas and semantic facts; malformed or mismatched evidence SHALL fail closed.

#### Scenario: Producer or input facts are corrupted
- **WHEN** a source identity, prototype, type, declaration binding or origin record differs from its authenticated relationships
- **THEN** validation SHALL reject it without semantic reanalysis or interning type IDs
- **AND** independent String, Buffer, opaque and file-snapshot counters SHALL return to their initial values

#### Scenario: Stage-one analysis is used without later integration
- **WHEN** only the independent source-origin layer has passed
- **THEN** it SHALL NOT claim general CFG/lifetime checking, HIR6/MIR support, scalar-copy completion or complete M3/self-hosting
