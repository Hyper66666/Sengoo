## ADDED Requirements

### Requirement: The language SHALL provide a complete integer type set with defined conversions

The language SHALL support signed and unsigned integer types of widths 8, 16,
32, and 64 plus pointer-sized `isize`/`usize`, with explicit casts and checked
conversions.

#### Scenario: Mixed-width arithmetic and conversion

- **WHEN** a program uses values of different integer widths and converts between
  them with `as` and with a checked conversion
- **THEN** `as` follows the documented truncation/sign rules
- **AND** the checked conversion returns an error status when the value does not
  fit the target type

### Requirement: Integer overflow behavior SHALL be defined

Arithmetic overflow SHALL trap in debug builds and wrap in release builds, and
explicit wrapping/checked/saturating operations SHALL be available in all builds.

#### Scenario: Overflowing addition by build mode

- **WHEN** an addition overflows the integer type
- **THEN** a debug build traps with a stable diagnostic and a release build wraps
  modulo the type width

#### Scenario: Explicit overflow-handling methods

- **WHEN** a program calls `wrapping_add`, `checked_add`, or `saturating_add`
- **THEN** wrapping wraps, checked returns `Option` (`none` on overflow), and
  saturating clamps to the type bounds, regardless of build mode

### Requirement: The language SHALL provide IEEE-754 floats with a math and format surface

`f32` and `f64` SHALL follow IEEE-754 semantics and SHALL have standard math
functions, parsing, and formatting.

#### Scenario: Float math and predicates

- **WHEN** a program computes `sqrt`, `pow`, and checks `is_nan`/`is_infinite`
- **THEN** results follow IEEE-754 and the predicates report correctly for
  `NaN`/`inf`

#### Scenario: Float parse and format round-trip

- **WHEN** a program parses a decimal float and then formats it with a precision
  spec such as `{:.3}`
- **THEN** parsing yields the correct value and formatting honors the precision

### Requirement: Numeric operators SHALL be trait-based, and literals SHALL support suffixes, bases, and separators

Arithmetic operators SHALL dispatch through numeric operator traits, and the
lexer SHALL accept typed suffixes, `0x`/`0o`/`0b` bases, and `_` digit
separators.

#### Scenario: Operator-trait dispatch

- **WHEN** an arithmetic operator is applied to a numeric type
- **THEN** it dispatches through the corresponding operator trait
  (`Add`/`Sub`/...)

#### Scenario: Literal forms lex to typed values

- **WHEN** a program writes `42i64`, `7u8`, `0o52`, `0b1010`, `1_000_000`, or
  `1.5f32`
- **THEN** each lexes to the documented value and numeric type
