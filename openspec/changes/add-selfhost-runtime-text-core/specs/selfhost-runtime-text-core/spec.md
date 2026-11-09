## ADDED Requirements

### Requirement: SG text validation SHALL enforce bounded UTF-8 scalar encoding

The SG implementation SHALL validate the complete bounded byte range without
delegating encoding policy to a C or Rust text implementation.

#### Scenario: A bounded input is validated
- **WHEN** the unsafe caller supplies a readable range of zero through 64 MiB
- **THEN** validation SHALL accept exactly complete shortest UTF-8 scalar encodings and reject malformed, truncated, surrogate and out-of-range sequences
- **AND** it SHALL preserve input bytes and allocate no owned runtime resource

#### Scenario: A length is invalid or empty
- **WHEN** the length is negative, exceeds the bound or equals zero
- **THEN** invalid lengths SHALL reject and empty input SHALL accept without reading the pointer

### Requirement: Byte-boundary queries SHALL validate input and offset

Boundary queries SHALL check complete input validity and the requested offset.

#### Scenario: A byte offset is queried
- **WHEN** a caller queries a bounded UTF-8 boundary
- **THEN** invalid text and offsets outside zero through length SHALL reject
- **AND** valid input SHALL accept zero/end and scalar starts while rejecting continuation positions

### Requirement: Text migration evidence SHALL preserve implementation ownership

Text migration reports SHALL distinguish SG algorithms from adapters and remaining runtime work.

#### Scenario: The SG text foundation passes
- **WHEN** independent execution qualifies the SG algorithm and minimal memory adapter
- **THEN** reports SHALL retain exact source/tool identity and the unsafe readable-extent contract
- **AND** this SHALL NOT count as owned-storage migration, final SDK packaging or complete self-hosting
