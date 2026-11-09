## ADDED Requirements

### Requirement: SG raw DEFLATE decoding SHALL support all defined block kinds

The SG decoder SHALL implement stored, fixed and dynamic Huffman raw DEFLATE
blocks without delegating compression policy to a C or Rust decoder.

#### Scenario: A bounded valid raw stream is decoded
- **WHEN** a caller supplies a complete raw stream and sufficient separate writable output
- **THEN** the decoder SHALL produce the exact bytes defined by its blocks, including overlapping and cross-block history references
- **AND** independent execution SHALL verify stored, fixed and dynamic streams

#### Scenario: A compressed stream is malformed or truncated
- **WHEN** a stream has an invalid block/tree/symbol/repeat/distance or lacks required bits
- **THEN** decoding SHALL return a malformed-stream failure before any unauthorized memory access
- **AND** it SHALL not report its partial output as a successful result

### Requirement: Raw decoding SHALL preserve bounded unsafe memory contracts

The caller SHALL supply valid, non-overlapping input/output extents within the
documented bound. The implementation SHALL retain neither pointer nor allocate
owned runtime storage.

#### Scenario: Limits or output capacity are invalid
- **WHEN** input/output limits are negative or oversized, or decoded bytes exceed output capacity
- **THEN** invalid limits SHALL reject without any memory access and capacity failure SHALL stop before the first out-of-range write
- **AND** protected-page execution SHALL verify input preservation and bounded partial output

### Requirement: Decoder evidence SHALL retain runtime and package boundaries

The evidence SHALL bind consumed SG/native SDK sources, tools, host and full
revision and distinguish the decoder from remaining package integration.

#### Scenario: The raw decoder passes its independent gate
- **WHEN** qualified SG emission and linking execute the raw decoder successfully
- **THEN** repeated fresh artifacts SHALL be byte-identical and reports SHALL record actual independent oracle comparisons
- **AND** the result SHALL not close gzip/tar/package compatibility, final SDK or full self-hosting milestones
