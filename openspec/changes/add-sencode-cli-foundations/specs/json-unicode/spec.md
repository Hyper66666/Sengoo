## ADDED Requirements

### Requirement: JSON strings SHALL decode Unicode scalar values losslessly

The existing JSON runtime SHALL decode Unicode escapes to UTF-8, combine a
valid UTF-16 surrogate pair into one supplementary scalar, and validate raw
UTF-8. Existing JSON public interfaces and limits SHALL remain compatible.

#### Scenario: Escaped and literal Unicode round trip

- **WHEN** JSON contains BMP escapes, supplementary surrogate pairs, control
  escapes, or valid literal UTF-8 through U+10FFFF
- **THEN** owned-string extraction preserves the exact decoded UTF-8 bytes
- **AND** serialization followed by parsing preserves those values

#### Scenario: Invalid Unicode is rejected

- **WHEN** a string contains an unpaired, reversed or invalid surrogate, short
  hex escape, invalid continuation, overlong UTF-8, UTF-8 surrogate or a value
  above U+10FFFF
- **THEN** parsing returns STATUS_PARSE (10) and a nonnegative error offset

### Requirement: Parsed embedded NUL values and keys SHALL retain their lengths

Parsed JSON strings and keys SHALL carry internal byte lengths. String copy,
owned-string extraction and serialization SHALL retain embedded U+0000.
Member comparison SHALL use length and bytes so a parsed `a\u0000b` key never
aliases `a`. Existing C-string builder inputs and key lookup inputs retain
their existing NUL-terminated boundary; this change does not extend that ABI.

#### Scenario: NUL string and object key serialize without truncation

- **WHEN** parsed JSON contains `"a\u0000b"` as a value or key
- **THEN** serialization retains the complete escaped value or key
- **AND** extraction of the value has three bytes
- **AND** lookup by `a` does not match the longer parsed key

### Requirement: Recursive JSON growth SHALL preserve parent node validity

Array and object parsing SHALL re-resolve parent node identity after recursive
child parsing that can relocate the document node array.

#### Scenario: Nested children exceed initial node storage

- **WHEN** an object containing an array of 300 nested objects is parsed
- **THEN** every child remains attached to the correct parent
- **AND** the final child is readable and serialization remains valid

### Requirement: JSON documents SHALL offer an independent bounded deep copy

`JsonDoc.copy(JsonValue)` SHALL copy same-document or cross-document values,
preserving exact integers, strings, NUL keys and scalar types. It SHALL reject
invalid source handles, cycles, excessive depth, node count or encoded bytes.
Failure SHALL roll back newly allocated destination nodes; success SHALL remain
valid after the source document closes. Existing setters SHALL still reject
foreign nodes unless the caller explicitly copies them.

#### Scenario: Independent nested JSON copy preserves exact values

- **WHEN** a caller copies nested data then drops its source document
- **THEN** the destination retains i64 extrema, Unicode, NUL bytes and arrays
- **AND** copying a value already in the destination remains safe during growth

#### Scenario: Failed deep copy leaves the destination intact

- **WHEN** a source is stale, cyclic or exceeds depth, 4096 nodes or 1 MiB
- **THEN** copying returns a stable error without altering existing values
- **AND** partial new nodes are removed so remaining node capacity is reusable
