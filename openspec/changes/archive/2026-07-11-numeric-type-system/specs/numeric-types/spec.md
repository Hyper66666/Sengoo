## ADDED Requirements

### Requirement: Numeric semantics SHALL have one production backend reference

LLVM-text plus clang SHALL be the production semantic reference for this
change. Experimental backends SHALL either produce the same result for accepted
programs or reject unsupported programs explicitly.

#### Scenario: Cranelift receives an unsupported numeric program

- **WHEN** the experimental Cranelift path receives calls, aggregates, floats,
  or another unsupported construct
- **THEN** it returns a documented unsupported diagnostic
- **AND** it does not silently reinterpret the program or define different
  source-language semantics

#### Scenario: Pointer-sized numeric code is cross-compiled

- **WHEN** `isize` or `usize` code is compiled for a target whose pointer width
  differs from the build host
- **THEN** parsing, range checks, MIR, casts, and production codegen use the
  selected target width

### Requirement: The language SHALL provide a complete integer type set with defined conversions

The language SHALL support signed and unsigned integer types of widths 8, 16,
32, and 64 plus pointer-sized `isize`/`usize`, with explicit casts and a v1
checked conversion family named
`checked_<source>_to_<target>(value) -> Result<Target, i64>`.

#### Scenario: Mixed-width arithmetic and conversion

- **WHEN** a program uses values of different integer widths and converts between
  them with `as` and `checked_<source>_to_<target>`
- **THEN** `as` follows the documented truncation/sign rules
- **AND** checked conversion returns `Result { is_ok: true, value, error: 0 }`
  when the value fits
- **AND** it returns `Result { is_ok: false, error: STATUS_OVERFLOW }` when the
  magnitude is out of range
- **AND** it returns `Result { is_ok: false, error: STATUS_INVALID_ARGUMENT }`
  for a negative-to-unsigned sign violation

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

Arithmetic operators SHALL dispatch through numeric operator traits. Binary
traits use the explicit `Trait<Rhs, Output>` model and unary negation uses
`Neg<Output>`; the final type parameter is the operator's output projection.
This is the source-level equivalent of an associated `Output` until qualified
`Self::Output` syntax is available. A concrete operator expression SHALL select
exactly one matching impl, and a generic operator expression SHALL preserve and
enforce the full `Rhs`/`Output` bound. Primitive numeric operators MAY lower to
intrinsics as compiler-known implementations of the same contract. The lexer
SHALL accept typed suffixes, `0x`/`0o`/`0b` bases, and `_` digit separators.

#### Scenario: Operator-trait dispatch

- **WHEN** an arithmetic operator is applied to a primitive or user-defined type
- **THEN** it dispatches through the corresponding operator trait
  (`Add`/`Sub`/...)
- **AND** missing, ambiguous, and mismatched output implementations produce
  stable diagnostics

#### Scenario: Literal forms lex to typed values

- **WHEN** a program writes `42i64`, `7u8`, `0o52`, `0b1010`, `1_000_000`, or
  `1.5f32`
- **THEN** each lexes to the documented value and numeric type
