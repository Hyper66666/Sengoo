# language-reference Specification

## Purpose
TBD - created by archiving change authoritative-language-reference. Update Purpose after archive.
## Requirements
### Requirement: An authoritative, versioned language reference SHALL match the implementation

The project SHALL maintain a single language reference that documents the
implemented language, with a per-construct status linked to proof, and SHALL
mark the legacy design draft historical.

#### Scenario: Reference claim matches the compiler

- **WHEN** the reference documents a language construct as Supported
- **THEN** a linked example or test demonstrates that construct compiling/running
- **AND** constructs that are not implemented are marked unsupported or removed,
  not presented as available

#### Scenario: Legacy draft is redirected

- **WHEN** a reader opens `Sengoo_Language_Specification.md`
- **THEN** it is marked historical and points to the authoritative reference

### Requirement: Reference examples SHALL be verified by CI doc-tests

Code blocks in the reference SHALL be compiled (and run where applicable) by CI
so the reference cannot drift from the compiler.

#### Scenario: A drifting reference example fails CI

- **WHEN** a reference code block no longer compiles or run-produces its
  documented result
- **THEN** the doc-test CI job fails
- **AND** the failure identifies the offending reference section

### Requirement: The reference SHALL be versioned with the toolchain

The reference SHALL declare which toolchain version it describes and follow a
documented versioning policy.

#### Scenario: Reference declares its version

- **WHEN** a user reads the reference
- **THEN** it states the toolchain version it corresponds to
- **AND** the versioning policy for updates is documented

### Requirement: Option and Result SHALL be enums with pattern constructors

`Option<T>` SHALL be `enum Option<T> { None, Some(T) }` and `Result<T, E>` SHALL
be `enum Result<T, E> { Ok(T), Err(E) }`. Their variants SHALL be usable as
value constructors and as `match` patterns without constructing a placeholder
payload for the absent or error case.

#### Scenario: An optional value is produced without a placeholder

- **WHEN** a function returning `Option<i64>` evaluates `Some(n)` on one path and
  `None` on another
- **THEN** type checking accepts both expressions
- **AND** no placeholder value is required to express `None`

#### Scenario: A fallible result is matched by variant

- **WHEN** a program matches a `Result<T, E>` with `Ok(value)` and `Err(error)`
  arms
- **THEN** each arm binds its payload with the variant's type
- **AND** the match is accepted as exhaustive without a wildcard arm

#### Scenario: Struct-literal construction is rejected with a migration hint

- **WHEN** a program constructs `Result { is_ok: true, value: v, error: e }` or
  `Option { is_some: false, value: p }`
- **THEN** type checking rejects the expression
- **AND** the diagnostic names the `Ok`/`Err`/`Some`/`None` replacement

#### Scenario: Compatibility accessors remain available with deprecation

- **WHEN** existing code reads `.is_ok`, `.is_some`, `.value`, or `.error` on
  these types during the compatibility release
- **THEN** the read resolves against the enum form and yields the same value as
  the previous struct layout
- **AND** a deprecation diagnostic reports the pattern-matching replacement

#### Scenario: Payloads are dropped exactly once per variant

- **WHEN** an `Option<T>` or `Result<T, E>` holding an owned payload goes out of
  scope, is moved out of by a `match` arm, or is propagated by `?`
- **THEN** the still-owned payload is dropped exactly once
- **AND** the non-selected variant's payload is not dropped

### Requirement: for loops SHALL iterate generic collections

`for pat in expr` SHALL accept generic collections and iterator values in
addition to the existing arrays, slices, and ranges.

#### Scenario: A collection is iterated directly

- **WHEN** a program writes `for value in items` where `items` is `Vec<T>`,
  `VecDeque<T>`, `HashSet<T>`, or `BTreeSet<T>`
- **THEN** the loop binds each element in the collection's iteration order

#### Scenario: An explicit iterator is iterated

- **WHEN** a program writes `for value in items.iter()` or iterates the result of
  a lazy adapter such as `map`, `filter`, `take`, `skip`, or `enumerate`
- **THEN** the loop consumes the iterator to completion

#### Scenario: A map is iterated by entry, key, or value

- **WHEN** a program iterates a `HashMap<K, V>` or `BTreeMap<K, V>` directly, or
  through `keys()` or `values()`
- **THEN** direct iteration binds entries, `keys()` binds keys, and `values()`
  binds values

#### Scenario: Array, slice, and range loops keep their existing lowering

- **WHEN** a program iterates a fixed array, a slice, or a range
- **THEN** the existing direct lowering is used
- **AND** no iterator-protocol indirection is introduced for those receivers

#### Scenario: Mutation during iteration stays rejected

- **WHEN** a program mutates a collection whose storage may move while a
  borrowing iteration over it is live
- **THEN** the existing borrow rules reject the program with the established
  diagnostic

### Requirement: if let SHALL bind a single pattern

The language SHALL support `if let PATTERN = EXPR { .. }` with an optional
`else` branch for refutable patterns.

#### Scenario: An optional value is unwrapped conditionally

- **WHEN** a program writes `if let Some(value) = maybe { .. } else { .. }`
- **THEN** the bound name is in scope in the matched branch only
- **AND** the else branch runs when the pattern does not match

#### Scenario: An irrefutable pattern is reported

- **WHEN** an `if let` pattern can never fail to match
- **THEN** a diagnostic reports the redundant conditional binding

### Requirement: Debug formatting SHALL render derived shapes

`{:?}` SHALL render values whose type derives `Debug`, in format arguments and
in f-string interpolation.

#### Scenario: A derived struct is debug-printed

- **WHEN** a `#[derive(Debug)]` struct is formatted with `{:?}` through
  `format`, `println`, or an f-string
- **THEN** the output names the type and its fields deterministically

#### Scenario: Debug formatting without the derive is rejected

- **WHEN** `{:?}` is applied to a type that does not derive or implement `Debug`
- **THEN** type checking rejects the format expression
- **AND** the diagnostic names the missing `Debug` derive

