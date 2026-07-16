## ADDED Requirements

### Requirement: The stdlib SHALL expose bounded Reader and Writer contracts

The stdlib SHALL expose synchronous `Reader.read_into`,
`Writer.write_buffer`, and `Writer.flush` traits with Buffer-based ownership,
partial-I/O, EOF, and stable-status semantics, plus bounded generic
`read_to_end`, `write_all`, and `copy_stream` helpers.

#### Scenario: Reader reaches EOF

- **WHEN** a Reader has no more bytes
- **THEN** `read_into` returns `Ok(0)`
- **AND** zero does not represent an error or an uninitialized Buffer

#### Scenario: Writer accepts only a prefix

- **WHEN** `write_buffer` accepts fewer than `used_len` bytes
- **THEN** it returns the accepted positive byte count
- **AND** `write_all` retries the remaining suffix
- **AND** repeated zero progress fails with a stable status instead of looping

#### Scenario: Destination capacity is exhausted

- **WHEN** `read_to_end` cannot fit more input in its caller-provided Buffer
- **THEN** it returns `STATUS_BUFFER_TOO_SMALL`
- **AND** does not grow memory implicitly or write beyond capacity

#### Scenario: Stream copy succeeds

- **WHEN** `copy_stream` reaches reader EOF using a caller-provided scratch
  Buffer
- **THEN** it returns the checked total bytes written
- **AND** uses no unbounded internal allocation

### Requirement: Existing I/O APIs SHALL remain source-compatible adapters

Supported file, stdio/owned-fd, and TCP types SHALL implement or adapt to the
Reader/Writer contracts without removing existing one-shot and concrete methods.

#### Scenario: Existing file read helper is used after M3

- **WHEN** a program calls an existing public file/Buffer helper
- **THEN** it compiles with unchanged success-path behavior
- **AND** its failure maps through the same positive status taxonomy

### Requirement: Sengoo text SHALL use a pinned Unicode 17.0.0 foundation

The stdlib SHALL declare Unicode version 17.0.0 for scalar properties and case
data, preserve UTF-8 byte-length/index semantics, and expose strict UTF-8
construction, char iteration/count, locale-independent simple case mapping, and
default case folding.

#### Scenario: String length and scalar count differ

- **WHEN** an owned or borrowed UTF-8 string contains multibyte scalars
- **THEN** `len()` returns bytes
- **AND** `char_count()` returns Unicode scalar count
- **AND** `chars()` yields `Iterator<Item = char>` in source order

#### Scenario: Invalid UTF-8 is converted to String

- **WHEN** `string_from_utf8(buffer, used_len)` receives malformed UTF-8
- **THEN** it returns `STATUS_INVALID_UTF8`
- **AND** does not create a partially valid String
- **AND** existing `string_from_buffer` follows the same strict validation

#### Scenario: Locale-independent case operation runs

- **WHEN** simple lower/upper mapping or `casefold` is requested
- **THEN** the result follows pinned Unicode 17.0.0 data
- **AND** output expansion uses checked bounded allocation
- **AND** no locale-specific behavior is implied

#### Scenario: Normalization or grapheme behavior is requested

- **WHEN** a user needs normalization, grapheme segmentation, collation, or
  locale-specific casing
- **THEN** v0.2 documentation marks it unsupported/follow-up
- **AND** byte or scalar operations are not mislabeled as equivalent

### Requirement: Invalid UTF-8 SHALL have one stable status category

The positive status namespace SHALL add `STATUS_INVALID_UTF8` with numeric value
`20`. It SHALL represent known malformed UTF-8 only; unrelated structured-data
syntax failures SHALL continue to use `STATUS_PARSE`.

#### Scenario: Runtime detects malformed UTF-8

- **WHEN** a public text-construction boundary proves that input bytes are not
  valid UTF-8
- **THEN** the wrapper returns error category `20`
- **AND** `status_name_copy` returns `invalid_utf8`
- **AND** `status_message_copy` returns a stable human-readable message
- **AND** existing status values `0` through `19` remain unchanged

### Requirement: The v0.2 stdlib profile SHALL include production HTTP evidence

M3 SHALL consume the archived `http-production-serving` requirements and SHALL
not claim the v0.2 production stdlib profile until handler routing, bounded
keep-alive, response streaming, and per-platform TLS server claims have their
required realworld/runtime evidence.

#### Scenario: A TLS stack lacks real handshake proof

- **WHEN** one supported host stack lacks the handshake evidence required by
  `http-production-serving`
- **THEN** the support matrix remains platform-specific for that stack
- **AND** M3 does not count plaintext fallback or disabled verification as proof
