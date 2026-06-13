## ADDED Requirements

### Requirement: `String` and `&str` SHALL be first-class UTF-8 text types

The language SHALL provide an owning, growable, UTF-8 `String` and a borrowed
`&str` view, where `String` is move-only and released automatically.

#### Scenario: Owned string needs no manual release

- **WHEN** a function creates a `String`, mutates it, and returns
- **THEN** the `String` is dropped automatically at scope end with no manual
  `.drop()` call
- **AND** a `String` always holds valid UTF-8

#### Scenario: Borrowed view does not own

- **WHEN** a `&str` is taken from a `String`
- **THEN** the `&str` can be read, compared, searched, and sliced
- **AND** the `&str` does not release the underlying buffer

### Requirement: Strings SHALL support ergonomic operators and methods

`String`/`&str` SHALL support concatenation, comparison, common query/transform
methods, and char/byte iteration.

#### Scenario: Concatenation and comparison

- **WHEN** a program writes `a + b` for `String` and `&str` operands and compares
  strings with `==` / `<`
- **THEN** concatenation yields a new `String` and comparison yields the expected
  boolean

#### Scenario: Query and iteration

- **WHEN** a program calls `contains`, `starts_with`, `split`, `trim`, or
  iterates `chars()` / `bytes()`
- **THEN** each returns results consistent with UTF-8 text semantics

### Requirement: Slicing SHALL respect UTF-8 char boundaries

Slicing or indexing a string SHALL operate on char boundaries and SHALL not
produce invalid UTF-8.

#### Scenario: Non-boundary slice is reported

- **WHEN** a program requests a slice whose start or end is not a char boundary
- **THEN** the fallible `get(a..b)` form returns a stable error status
- **AND** the infallible slice form fails deterministically rather than producing
  invalid UTF-8

### Requirement: Formatting SHALL be trait-based and print any `Display`

The language SHALL provide `Display`/`Debug` formatting, a `format` function with
a documented mini-language, and `print`/`println`/`eprintln` that accept any
`Display` value.

#### Scenario: Print a string and a struct

- **WHEN** a program calls `println` with a `String` and with a struct that
  derives `Debug` using `{:?}`
- **THEN** the string prints as its text and the struct prints via its `Debug`
  formatting

#### Scenario: Integer printing stays compatible

- **WHEN** existing code calls `print(<i64>)`
- **THEN** it continues to compile and print the integer because integers
  implement `Display`

#### Scenario: Malformed format literal is a compile error

- **WHEN** a `format`/interpolation literal has mismatched arity or an invalid
  spec
- **THEN** compilation fails with a stable diagnostic, since the literal is known
  at compile time

### Requirement: String interpolation and extended literals SHALL be supported

The lexer/parser SHALL support `f"..."` interpolation, `b"..."` byte strings,
`"""..."""` multiline strings, and integer literal bases `0o`/`0b` plus typed
suffixes.

#### Scenario: Interpolation lowers to format

- **WHEN** a program writes `f"x={x} y={y:?}"`
- **THEN** it lowers to the equivalent `format("x={} y={:?}", x, y)` and produces
  the same output

#### Scenario: Extended literals lex correctly

- **WHEN** a program uses `b"..."`, `"""..."""`, `0o52`, `0b101010`, or a typed
  suffix such as `42i64`
- **THEN** each literal lexes to the documented value and type
- **AND** multiline literals strip common leading whitespace as documented
