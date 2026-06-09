## ADDED Requirements

### Requirement: Stdlib text-producing helpers SHALL return owned String values

Owned `String` SHALL become an accepted stdlib return ABI for the additive
helpers introduced by `stdlib-production-surface`. Callers SHALL not be required
to preallocate a `Buffer` for those helpers.

#### Scenario: Stdlib returns String by value for pinned helpers

- **WHEN** a program calls an additive stdlib helper listed in the
  `stdlib-mainstream-usability` production-helper table
- **THEN** success returns `String` or `Result<String, i64>` according to that helper
- **AND** allocation failure returns `STATUS_OUT_OF_MEMORY` in the public positive
  status namespace

#### Scenario: String results remain compatible with explicit Buffer copies

- **WHEN** a program needs to copy an owned `String` into a managed `Buffer`
- **THEN** it uses `copy_to_buffer` from the canonical owned-string specification
- **AND** `Buffer.len()` capacity semantics remain unchanged

#### Scenario: sglsp and examples expose owned-return signatures

- **WHEN** `sglsp` analyzes imports that use owned-return helpers
- **THEN** completion and hover show `Result<String, i64>` or `String` as specified
- **AND** at least one stdlib or realworld example uses owned-return helpers without
  raw `ffi_buffer_*` in application code
