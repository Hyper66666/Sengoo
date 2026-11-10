## ADDED Requirements

### Requirement: Incremental DEFLATE SHALL use fixed bounded SG continuation state

The continuation decoder SHALL preserve bit, block, Huffman, repeat, match
and 32 KiB history state in fixed caller-owned storage, retain no input/output
pointer and implement all compression policy in SG. A minimal C adapter SHALL
provide only primitive memory access. Init/reset/dispose SHALL allocate nothing.

#### Scenario: Arbitrary input and output chunk cuts
- **WHEN** a valid stored, fixed or dynamic stream is presented at arbitrary or single-byte cuts with independently bounded output chunks
- **THEN** exact consumed/produced counters and need-input/need-output/finished states SHALL permit byte-identical completion
- **AND** overlapping and cross-block references SHALL use only previously initialized ring history

#### Scenario: Final input and output backpressure interact
- **WHEN** finish is asserted with pending output, zero capacity or an incomplete symbol
- **THEN** finish SHALL remain sticky across retries, pending bytes SHALL not be lost and final missing bits SHALL produce truncation
- **AND** zero input SHALL still permit draining buffered matches

#### Scenario: Exact and prefix completion remain distinct
- **WHEN** final EOB is followed by unused padding bits and complete framing bytes
- **THEN** prefix mode SHALL return the precise raw byte boundary without reading framing bytes
- **AND** exact mode SHALL reject complete trailing bytes after final EOF is asserted

#### Scenario: Invalid extents and state ownership
- **WHEN** an invalid extent, flag or query field is supplied
- **THEN** the call SHALL return its precise API error without accessing caller memory
- **AND** documented reset/copy/dispose behavior SHALL not hide a heap allocation or external handle

#### Scenario: Migration to the existing exports
- **WHEN** the new continuation prototype passes its chunked gate
- **THEN** this SHALL NOT by itself qualify replacement of the existing raw/gzip exports
- **AND** their final shared-core migration SHALL require fresh complete raw and gzip regression qualifications

### Requirement: Existing one-shot exports SHALL use the single continuation core

The raw exact and prefix compatibility exports SHALL use the same SG
bit/Huffman/LZ implementation as the continuation API, with bounded SG-owned
temporary state and explicit complete native object dependencies. Existing
64 MiB limits, shift-27 counts and failure behavior SHALL remain compatible.

#### Scenario: One-shot lifetime and allocation failure
- **WHEN** an export acquires its fixed 45,056-byte state or allocation fails
- **THEN** SG SHALL dispose and release every acquired state on success and failure, and allocation failure SHALL return -2 before raw input/output access
- **AND** independent real OS memory observations SHALL verify exact commit size, erased bytes, zero live allocations and actual release

#### Scenario: Gzip raw-state allocation fails
- **WHEN** gzip has read its valid fixed ten-byte header and its raw-state allocation fails
- **THEN** the gzip call SHALL return -2 without reading raw DEFLATE bytes or writing output
- **AND** the evidence SHALL distinguish header reads from zero-access raw/prefix controls

#### Scenario: A native caller omits the shared core object
- **WHEN** the synchronous wrapper is linked without its continuation core dependency
- **THEN** the qualified SG linker SHALL reject unresolved symbols and SHALL NOT publish an executable
- **AND** deployment documentation SHALL list all required SG and memory-provider objects

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

#### Scenario: Unused distance declarations differ from zlib limits
- **WHEN** an otherwise valid dynamic empty stream declares 31 or 32 distance lengths and decodes no distance
- **THEN** the RFC header range SHALL be supported while decoded reserved distance symbols remain errors
- **AND** zlib's stricter header rejection SHALL remain an explicit independent-oracle difference

#### Scenario: A deployed duplicate length encoding is read
- **WHEN** length symbol 284 has extra value 31
- **THEN** the decoder SHALL preserve deployed decoder compatibility by producing length 258
- **AND** tests SHALL distinguish this duplicate from 284 extra 30 and canonical symbol 285

### Requirement: Raw decoding SHALL preserve bounded unsafe memory contracts

The caller SHALL supply valid, non-overlapping input/output extents within the
documented bound. The implementation SHALL retain neither pointer nor allocate
owned runtime storage.

#### Scenario: Limits or output capacity are invalid
- **WHEN** input/output limits are negative or oversized, or decoded bytes exceed output capacity
- **THEN** invalid limits SHALL reject without accessing either supplied range and capacity failure SHALL stop before the first out-of-range write
- **AND** protected-page execution SHALL verify input preservation and bounded partial output

#### Scenario: A complete final block has trailing bytes
- **WHEN** the final end-of-block has unused bits in its last consumed byte or additional complete supplied bytes
- **THEN** unused final-byte padding SHALL be allowed and any additional complete bytes SHALL return malformed-stream failure
- **AND** no framed consumed-length API SHALL be inferred from that raw exact-input contract

### Requirement: Decoder evidence SHALL retain runtime and package boundaries

The evidence SHALL bind consumed SG/native SDK sources, tools, host and full
revision and distinguish the decoder from remaining package integration.

#### Scenario: The raw decoder passes its independent gate
- **WHEN** qualified SG emission and linking execute the raw decoder successfully
- **THEN** repeated fresh artifacts SHALL be byte-identical and reports SHALL record actual independent oracle comparisons
- **AND** the result SHALL not close gzip/tar/package compatibility, final SDK or full self-hosting milestones

### Requirement: Gzip framing SHALL reuse the raw decoder and verify member integrity

The SG gzip entry SHALL reuse one shared raw algorithm through an exact consumed
byte-count contract. It SHALL parse bounded RFC 1952 headers and optional fields
and validate header CRC16, output CRC32 and ISIZE when present or required.

#### Scenario: A valid optional header and trailer are decoded
- **WHEN** a complete member contains FEXTRA, FNAME, FCOMMENT or FHCRC
- **THEN** the SG decoder SHALL locate the raw stream without losing bytes and verify the applicable checksums and size
- **AND** the copied result SHALL match independent decoder output without retaining either pointer

#### Scenario: A member is truncated or its integrity fields are corrupt
- **WHEN** a required optional-field terminator, raw final block or trailer is missing, or a header/body checksum or ISIZE is wrong
- **THEN** the framing entry SHALL return malformed failure and SHALL NOT publish its partial output as success

#### Scenario: Member and tail policies are explicit
- **WHEN** mode zero, strict-single mode one or strict-concatenated mode two is selected
- **THEN** mode zero SHALL fully validate the first member and return its wire consumption, mode one SHALL require immediate EOF, and mode two SHALL require at least one member and decode every adjacent member to EOF
- **AND** empty input and trailing zero padding SHALL NOT be silently accepted by either strict mode

#### Scenario: Header metadata reaches the consumer bound
- **WHEN** a name or comment exceeds 65,535 non-NUL bytes or an optional field exceeds the available input
- **THEN** the selected flate2-compatible entry SHALL fail within its authorized extents
- **AND** independent-oracle differences from more tolerant Python policies SHALL remain explicitly recorded
