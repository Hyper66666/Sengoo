## ADDED Requirements

### Requirement: Match exhaustiveness and reachability SHALL be specified and enforced

Enum and bool matches SHALL be exhaustive; matches over open-ended domains SHALL
contain an unguarded wildcard to be exhaustive. Guarded arms SHALL NOT remove a
constructor from the remaining coverage set, and arms after complete unguarded
coverage SHALL be rejected as unreachable.

#### Scenario: Guarded enum arm does not complete coverage

- **WHEN** an enum variant appears only in an arm with a guard
- **THEN** a match without another unguarded arm for that variant or `_` fails
  with `non-exhaustive-match`

#### Scenario: Arm follows wildcard

- **WHEN** an arm appears after an unguarded wildcard arm
- **THEN** type checking fails with `unreachable-match-arm`

### Requirement: Match payload bindings SHALL preserve ownership

By-value payload bindings SHALL move non-Copy payload fields; borrowed bindings
SHALL keep the matched owner borrowed for the inferred region.

#### Scenario: Payload moves from an enum

- **WHEN** an arm binds an owning payload by value
- **THEN** the payload is moved exactly once
- **AND** Drop does not release it through both the enum and the binding

### Requirement: Fixed arrays SHALL have a complete v0.2 semantic contract

Fixed arrays SHALL have compile-time length, checked indexing, deterministic
left-to-right iteration, whole-value move semantics for non-Copy elements, and
reverse-index Drop. Indexed partial moves SHALL be rejected.

#### Scenario: Constant index is out of bounds

- **WHEN** a compile-time-known index is outside the fixed length
- **THEN** compilation fails with `array-index-out-of-bounds`

#### Scenario: Runtime index is out of bounds

- **WHEN** a runtime index is outside the fixed length
- **THEN** execution follows the documented bounds-failure policy
- **AND** does not read or write outside the array

### Requirement: Structured exits SHALL preserve cleanup and expression results

`return`, `?`, `break`, and `continue` SHALL lower to explicit control-flow exits
that preserve expression values and drop all live owning paths exactly once.

#### Scenario: Loop exits with live owners

- **WHEN** `break` or `continue` leaves a scope containing live owning locals
- **THEN** locals leaving scope are dropped in reverse declaration order
- **AND** owners whose scope continues are not dropped early
