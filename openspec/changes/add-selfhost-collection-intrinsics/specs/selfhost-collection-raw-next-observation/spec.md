## ADDED Requirements

### Requirement: A direct borrowed-next call SHALL preserve its proved iterator owner during argument evaluation

The fresh ownership entry SHALL treat only the exact certified iterator-handle
argument of an admitted direct borrowed-next call as an observation. The query
alone SHALL NOT authorize a reference or replace independent source/provider
replay. Ownership state and action encodings SHALL remain unchanged.

#### Scenario: A known iterator is advanced twice
- **WHEN** fresh physical modules, external pins and actual typed facts prove both direct calls use the same live canonical iterator family and element
- **THEN** reading each handle SHALL preserve its owner and final typed cleanup
- **AND** the first read SHALL NOT emit a move or disarm action for the iterator
- **AND** actual machine execution SHALL restore the original opaque resource count

#### Scenario: A call or candidate relationship is forged
- **WHEN** the actual argument, callable, physical source, field, family, element, backing root, token, actual-call event or selected provider relationship differs
- **THEN** the argument SHALL NOT gain the observation contract
- **AND** fresh replay SHALL reject altered candidate tables or independent pins
- **AND** conditional or unknown handles SHALL NOT become known runtime references

#### Scenario: Observing a handle does not validate a payload use
- **WHEN** source reads a mutable, unguarded, escaping or invalidated optional payload
- **THEN** source ownership and final physical payload validation SHALL continue to reject
- **AND** a faithful candidate SHALL remain unlowerable until a separate fresh source entry proves the complete lowering context

#### Scenario: Push consumes a non-Copy argument
- **WHEN** the same function also invokes an independently admitted push
- **THEN** the push argument SHALL retain its exact consuming contract
- **AND** subsequent source use of that moved value SHALL reject
- **AND** the iterator-handle observation SHALL NOT generalize to arbitrary i64 fields, unknown calls or owner projections
