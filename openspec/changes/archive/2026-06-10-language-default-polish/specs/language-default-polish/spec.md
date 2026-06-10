## ADDED Requirements

### Requirement: Additive language polish SHALL not duplicate delivered P4 work

The change SHALL own only remaining additive language-surface relaxations and
diagnostic parity after existing language changes have landed.

#### Scenario: Delivered language surfaces are not reopened

- **WHEN** implementation plans work on `?`, `try {}`, match exhaustiveness,
  owned `String`, class header trait lists, the phase 4a attribute matrix, or
  dynamic native i64 FFI arity `0..=8`
- **THEN** the plan cites the existing owning change instead of re-specifying the
  delivered behavior
- **AND** this change only adds adjacent tests or diagnostic parity when needed

### Requirement: Remaining restrictions SHALL be inventoried before relaxation

The implementation SHALL record the current rejection path and add negative
tests for still-rejected adjacent forms before relaxing a parser, typeck,
lowering, async-frame, FFI, attribute, match/try, or diagnostic restriction.

#### Scenario: A source form remains unsupported

- **WHEN** a candidate form such as unsupported `cfg`, generic extern, aggregate
  FFI signature, owned `String` FFI signature, callback signature, mutable
  reference FFI signature, payload enum across await, or a new match/try pattern
  remains unsupported
- **THEN** compiler tests assert the rejection
- **AND** the rejection has a stable diagnostic code or documented stable message
  prefix
- **AND** `sglsp` reports the same source range, severity, and code/message
  family

#### Scenario: A source form becomes supported

- **WHEN** a remaining restriction is relaxed
- **THEN** parser and typeck tests cover the accepted source form
- **AND** lowering or runtime-shape tests cover executable behavior where
  applicable
- **AND** adjacent unsupported forms continue to fail with negative tests

### Requirement: Attribute polish SHALL preserve explicit target and site rules

Attribute expansions beyond the existing phase 4a matrix SHALL be accepted only
when their grammar, declaration-site matrix, false-filtering behavior, and
diagnostics are pinned.

#### Scenario: A new cfg predicate is accepted

- **WHEN** a future implementation accepts a `cfg` predicate beyond
  `target_os = "..."`
- **THEN** the accepted predicate set is limited to `target_os`, `target_family`,
  `feature`, `all(...)`, `any(...)`, and `not(...)`
- **AND** false predicates remove the declaration before type checking
- **AND** malformed predicates and unsupported targets produce stable compiler
  and `sglsp` diagnostics

#### Scenario: Feature cfg has no standalone command-line selector

- **WHEN** a standalone source file uses `#[cfg(feature = "...")]`
- **THEN** the predicate evaluates false unless a future OpenSpec accepts a
  feature-selection CLI flag
- **AND** package mode reads feature availability only from the selected manifest
  model

#### Scenario: Deprecated diagnostics stay warning-only

- **WHEN** source uses a declaration marked `#[deprecated]` or
  `#[deprecated("message")]`
- **THEN** `sgc` emits the stable `attributes::deprecated_use` warning
- **AND** `sglsp` mirrors severity, code, message, and source range

#### Scenario: An attribute remains unsupported

- **WHEN** a program uses an unsupported attribute name or an accepted attribute
  on a disallowed declaration kind
- **THEN** the compiler reports a stable attribute diagnostic at the attribute
  source range
- **AND** `sglsp` mirrors the diagnostic without suggesting an unsafe rewrite

### Requirement: FFI source signature polish SHALL stay explicit and bounded

FFI source-level relaxations SHALL be accepted only for pinned ABI and type
shapes. Generic extern functions, aggregate values, owned `String`, callbacks,
mutable references, and unsupported ABI names SHALL remain rejected unless this
change adds explicit accepted scenarios and tests for them.

This phase SHALL NOT widen the accepted FFI type set. It owns diagnostics and
negative tests for rejected neighbors.

#### Scenario: Unsupported FFI source shape is rejected

- **WHEN** source declares a generic extern function, unsupported ABI, aggregate
  FFI parameter or return, owned `String` FFI parameter or return, callback
  signature, mutable reference, or raw-pointer boundary without `unsafe`
- **THEN** type checking rejects the signature with a stable diagnostic
- **AND** `sglsp` reports the same code/message family at the signature site

#### Scenario: A new FFI source shape is accepted

- **WHEN** a future implementation accepts a new FFI source signature shape
- **THEN** call-shape validation is covered by typeck and lowering or runtime
  tests
- **AND** unsupported neighboring shapes still have negative tests

### Requirement: Async frame polish SHALL handle payload values intentionally

Payload-carrying enum values crossing `await` SHALL either be accepted with
frame layout/load/store coverage or remain rejected with stable diagnostics.

#### Scenario: Payload enum crosses await

- **WHEN** a future implementation accepts a payload-carrying enum local,
  parameter, or return value across an await point
- **THEN** async frame slot layout, store/load, lowering, and native async tests
  prove the payload is preserved
- **AND** ownership/drop behavior is covered for success and cancellation paths
- **AND** no half-lowered payload enum reaches LLVM without frame proof

#### Scenario: Payload enum crossing await remains deferred

- **WHEN** payload-carrying enum values still cannot cross await points
- **THEN** compiler and `sglsp` diagnostics name the async-frame restriction
- **AND** tests prevent a panic, ICE, or generic lowering failure from replacing
  the diagnostic

### Requirement: Breaking cleanup SHALL require migration documentation

This change SHALL NOT implement source-incompatible cleanup unless a migration
document exists and the parent umbrella accepts it.

#### Scenario: A cleanup would reject previously accepted source

- **WHEN** implementation discovers a syntax, attribute, FFI, async-frame,
  match/try, or diagnostic cleanup that changes accepted source behavior
- **THEN** implementation stops before code changes for that cleanup
- **AND** a migration document records old behavior, new behavior, replacement
  source, diagnostic code/text, compatibility window, and examples
- **AND** parent integration explicitly accepts the migration before archive
