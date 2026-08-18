## ADDED Requirements

### Requirement: Binary Buffer helpers SHALL support bounded framed protocols

The standard library SHALL expose byte get/set and unsigned big-endian 32-bit read/write operations over managed `Buffer` values. Every operation SHALL validate handles, offsets, lengths, value ranges, and arithmetic overflow before accessing memory.

#### Scenario: A frame length round-trips

- **WHEN** a program writes a `u32` value in big-endian form at a valid Buffer offset and reads it back
- **THEN** the original value is returned independent of host endianness
- **AND** the four stored bytes use network byte order

#### Scenario: A byte access is out of bounds

- **WHEN** byte or `u32` access uses a negative offset, insufficient remaining capacity, an out-of-range byte/value, or overflowing offset arithmetic
- **THEN** the helper returns a stable invalid-argument or overflow status
- **AND** no Buffer byte is read or modified outside its bounds

### Requirement: Standard I/O SHALL provide exact binary transfer helpers

The standard library SHALL provide offset-aware exact-read and write-all helpers for managed Buffer ranges on stdin/stdout and SHALL expose deterministic clean-EOF, truncated-input, I/O-error, and success outcomes. Protocol users SHALL be able to initialize standard streams in binary mode; on Windows this SHALL configure stdin and stdout with `_O_BINARY` before transferred bytes are interpreted.

#### Scenario: Exact input arrives in partial reads

- **WHEN** an exact read of a valid Buffer range is satisfied by multiple short native reads
- **THEN** the helper advances the offset until the requested byte count is filled and reports success

#### Scenario: EOF occurs before or during an exact read

- **WHEN** EOF occurs before any byte of a requested prefix or after only part of the requested range
- **THEN** the helper distinguishes clean EOF from truncated input with stable documented outcomes
- **AND** it never reports the truncated range as complete

#### Scenario: Output requires partial writes

- **WHEN** a valid Buffer range is written through native calls that accept fewer bytes than requested
- **THEN** the write-all helper continues from the correct offset until complete or a stable error occurs
- **AND** a zero-progress or failed write is not treated as success

#### Scenario: Windows pipes preserve all byte values

- **WHEN** a Windows program enables binary protocol I/O and transfers prefixes/payloads containing `0x0a`, `0x0d`, and `0x1a` through real parent/child pipes
- **THEN** stdin/stdout bytes match exactly without newline translation or control-Z EOF behavior

#### Scenario: Existing text-style I/O remains compatible

- **WHEN** a program continues to call existing `io_stdin_read`, `io_stdin_read_line`, `io_stdout_write`, or flush helpers without opting into the new protocol helpers
- **THEN** its documented source signatures and behavior remain unchanged

### Requirement: Strict JSON parsing SHALL preserve object and Unicode validity

The standard library SHALL add an opt-in strict JSON parse surface that validates the declared input length, UTF-8, complete grammar consumption, configured nesting bound, integer range, Unicode escape and surrogate-pair correctness, and uniqueness of decoded object keys. Existing `json_parse` and `json_parse_buffer` behavior SHALL remain source-compatible.

#### Scenario: A strict valid Unicode document parses

- **WHEN** strict parsing receives valid UTF-8 and JSON escapes including non-ASCII BMP characters and valid surrogate pairs within configured bounds
- **THEN** decoded strings contain the corresponding Unicode scalar values encoded as UTF-8
- **AND** serialization/reparse preserves their semantic values

#### Scenario: Duplicate decoded keys are rejected

- **WHEN** one object contains the same decoded key more than once, including equivalent literal and escaped spellings
- **THEN** strict parsing fails with a stable duplicate-field or parse status
- **AND** it never silently keeps the first or last value

#### Scenario: Malformed text is rejected

- **WHEN** input contains invalid UTF-8, an invalid escape, an unpaired surrogate, a control character, trailing non-whitespace, excess nesting, or an integer outside the supported exact range
- **THEN** strict parsing fails deterministically without panic or partial-document success

#### Scenario: Permissive callers do not change silently

- **WHEN** an existing program uses the pre-change JSON parse entry points
- **THEN** its accepted input and observable result remain governed by the existing specification
- **AND** strict behavior requires an explicit new entry point or option

### Requirement: JSON object inspection SHALL enable exhaustive decoders

Strictly parsed object values SHALL expose bounded key count, indexed decoded-key access, exact key lookup, and value-kind inspection so application code can enforce required and allowed fields. Key equality SHALL use decoded Unicode scalar/UTF-8 equality without normalization or case folding.

#### Scenario: An application rejects an unknown field

- **WHEN** an exhaustive decoder iterates a strictly parsed object and encounters a key outside its contract allowlist
- **THEN** it can return a stable unknown-field error before consuming the object as a domain value

#### Scenario: Object keys are inspected safely

- **WHEN** code requests a key at a valid or invalid index
- **THEN** a valid index returns an owned decoded key and an invalid index returns a stable out-of-range status
- **AND** no borrowed runtime pointer escapes the JSON handle lifetime

#### Scenario: General schema validation remains out of scope

- **WHEN** a future application needs a reusable JSON Schema dialect, streaming validation, or dynamic Sengoo object mapping
- **THEN** it first updates OpenSpec with the dialect, lifecycle, resource ceilings, ownership, and compatibility rules
- **AND** the strict parse and object-inspection subset remains usable without that feature

### Requirement: Strict JSON failures SHALL expose stable machine-readable kinds

The strict JSON surface SHALL expose a stable error kind that distinguishes
unclassified syntax failures, duplicate decoded object keys, invalid
UTF-8/Unicode escapes or surrogates, and trailing input. Existing parse status,
offset, and human-readable message APIs SHALL remain compatible. Protocol code
SHALL NOT need to branch on diagnostic text.

#### Scenario: A protocol maps strict parse failures

- **WHEN** strict parsing rejects duplicate keys, invalid Unicode, or trailing bytes
- **THEN** the caller receives the documented stable error kind for that category
- **AND** the legacy parse status remains `PARSE` with its existing offset/message behavior

#### Scenario: A later JSON operation succeeds

- **WHEN** any prior parse failed and a subsequent parse succeeds
- **THEN** the last-error kind resets to `NONE`

### Requirement: JSON builders SHALL preserve explicit string byte lengths

The JSON document builder SHALL provide a length-aware string creation path
that copies exactly the declared valid UTF-8 bytes, including embedded NUL,
without changing the existing C-string-compatible helper.

#### Scenario: A decoded string containing U+0000 is echoed

- **WHEN** a strict document decodes `\u0000` and a caller creates a new JSON string from its owned bytes and explicit length
- **THEN** serialization and strict reparse preserve the complete string value and byte length
- **AND** no suffix after the embedded NUL is truncated
