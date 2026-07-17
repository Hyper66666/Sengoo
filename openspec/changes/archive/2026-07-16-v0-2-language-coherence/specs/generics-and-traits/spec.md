## ADDED Requirements

### Requirement: Associated type projections SHALL resolve in all pinned type positions

`Self::Assoc` and `T::Assoc` SHALL resolve in trait declarations, impls, generic
signatures, return types, local annotations, and nested generic arguments when a
unique bound/impl defines the associated type.

#### Scenario: Nested projection resolves

- **WHEN** a generic function uses `Option<T::Item>` under a unique
  `T: Iterator` bound
- **THEN** type checking and monomorphization substitute the concrete associated
  type throughout the nested type

#### Scenario: Projection is not uniquely bound

- **WHEN** no bound or multiple incompatible bounds can define a projection
- **THEN** type checking fails with `unresolved-associated-type`

### Requirement: Traits SHALL support receiver-less methods with unambiguous path calls

A trait SHALL be able to declare a method without `self`. Such a method SHALL be
callable as `Trait::method(args)` or `Type::method(args)` when arguments and
expected result select exactly one implementation.

#### Scenario: Expected type selects a From-style implementation

- **WHEN** `let output: Target = From::from(input)` has exactly one applicable
  implementation
- **THEN** the call resolves and monomorphizes that implementation

#### Scenario: Associated call is ambiguous

- **WHEN** more than one implementation remains applicable after argument and
  expected-type inference
- **THEN** type checking fails with
  `ambiguous-trait-associated-function`
- **AND** the diagnostic lists the candidate trait/type pairs

### Requirement: The supported derive set SHALL be structural and ownership-safe

Derive expansion for the documented v0.2 set SHALL cover supported named structs
and enums, recurse through fields/payloads, preserve generic bounds, and reject
Copy for types that implement Drop or contain non-Copy fields.

#### Scenario: Generic enum derives structural traits

- **WHEN** a generic payload enum derives supported comparison, clone, hash,
  default, or debug traits
- **THEN** generated impls add only the required field bounds
- **AND** every variant participates in the derived behavior
