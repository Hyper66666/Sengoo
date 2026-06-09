# language-surface-expansion Specification

## Purpose
TBD - created by archiving change language-surface-expansion. Update Purpose after archive.
## Requirements
### Requirement: Supported attributes parse on listed declaration kinds

The compiler SHALL accept only the following attribute matrix in phase 4a:

| Attribute | struct | enum | class | trait | impl | fn/method | const |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `#[derive(...)]` | yes | yes | yes | no | no | no | no |
| `#[cfg(target_os = "...")]` | yes | yes | yes | yes | yes | yes | yes |
| `#[deprecated]` / `#[deprecated("message")]` | yes | yes | yes | yes | no | yes | yes |

Additional rules:

- `cfg` accepts only `target_os` predicates supported by the compiler target model.
- A false `cfg(target_os = "...")` removes the declaration before type checking.
- `deprecated` emits a stable warning in both `sgc` and `sglsp`.
- Unsupported attributes produce diagnostics naming the attribute and site.

#### Scenario: Allowed attributes lower successfully

- **WHEN** a program uses an allowed attribute from the matrix on a supported
  declaration kind
- **THEN** parsing and lowering succeed

#### Scenario: Unsupported attributes fail with stable diagnostics

- **WHEN** a program uses an attribute outside the matrix or on a disallowed
  declaration kind
- **THEN** the compiler reports a stable attribute diagnostic with site information

### Requirement: Class header trait lists parse with explicit base and trait rules

The parser and type checker SHALL distinguish class bases from implemented traits
using the first resolved path kind and reject invalid header orderings.

#### Scenario: Class with base and traits parses correctly

- **WHEN** a declaration uses `class Child: Base, TraitA, TraitB`
- **THEN** the parser records `extends = Base` and `implements = [TraitA, TraitB]`
- **AND** type checking validates the base and trait references
- **AND** trait method dispatch tests pass

#### Scenario: Trait-only class headers parse correctly

- **WHEN** a declaration uses `class Service: TraitA, TraitB` and the first resolved
  path is a trait
- **THEN** the class has no base and all listed paths are implemented traits

#### Scenario: Invalid header orderings are rejected

- **WHEN** a class header lists a class path after a trait path or more than one
  class base
- **THEN** type checking fails with a stable diagnostic

### Requirement: Dynamic native i64 FFI supports arity zero through eight

The dynamic native i64 call ABI SHALL support argument arity `0..=8` on supported
hosts while continuing to reject unsupported aggregate, owned-string, and callback
signatures.

#### Scenario: Supported arities compile and link

- **WHEN** a program uses the dynamic native i64 call ABI with arity `0..=8`
- **THEN** compilation and native linking succeed on supported hosts

#### Scenario: Unsupported signatures remain blocked

- **WHEN** a program uses aggregate values, owned `String`, callback signatures, or
  arity greater than eight
- **THEN** the compiler or runtime returns a compile-time error or `STATUS_UNSUPPORTED`
- **AND** existing `runtime-hardening-ffi-async` negative tests still pass

