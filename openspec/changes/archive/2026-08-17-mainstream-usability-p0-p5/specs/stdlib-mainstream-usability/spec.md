## ADDED Requirements

### Requirement: Option and Result helpers SHALL expose an enum-shaped surface

Standard-library `Option`/`Result` helpers SHALL operate on the enum form, SHALL
NOT require a placeholder payload to express absence or failure, and SHALL keep
their existing names working during the compatibility release.

#### Scenario: Absence is constructed without a placeholder

- **WHEN** library code produces an empty `Option<T>` or a failed
  `Result<T, E>`
- **THEN** `None` and `Err(error)` are sufficient
- **AND** no value of `T` is required to build them

#### Scenario: Placeholder constructors are deprecated

- **WHEN** existing code calls `option_none_with(placeholder)` or a
  `result_*_with(placeholder, ..)` constructor
- **THEN** the call still produces the documented value during the compatibility
  release
- **AND** a deprecation diagnostic names the constructor replacement

#### Scenario: Fallible wrappers return matchable values

- **WHEN** a fallible stdlib wrapper reports failure through the `std::status`
  taxonomy
- **THEN** the returned `Result` is matchable with `Ok`/`Err`
- **AND** the error payload carries the same status category as before

#### Scenario: Sentinel aggregates are no longer required to read handles

- **WHEN** user code reads an optional handle-shaped value such as a JSON value,
  buffer, or document
- **THEN** the value is obtained by matching or by a defaulting helper
- **AND** constructing a zero-initialised aggregate to satisfy `unwrap_or` is not
  required

### Requirement: Collections SHALL provide native iteration entry points

Runtime-backed collections SHALL be iterable by `for` and SHALL expose map key
and value iteration.

#### Scenario: Sequence and set collections iterate

- **WHEN** a program iterates `Vec<T>`, `VecDeque<T>`, `HashSet<T>`, or
  `BTreeSet<T>` with `for`
- **THEN** iteration yields the collection's elements in its documented order

#### Scenario: Maps expose keys and values

- **WHEN** a program calls `keys()` or `values()` on `HashMap<K, V>` or
  `BTreeMap<K, V>`
- **THEN** the returned iterator yields keys or values respectively
- **AND** `BTreeMap` order is deterministic

#### Scenario: Existing iterator terminals keep working

- **WHEN** existing code uses `count`, `fold`, `collect`, `sum`,
  `collect_hashset`, or `collect_hashmap`
- **THEN** those terminals continue to behave as documented

### Requirement: Vector literals SHALL have a pinned built-in form

The standard library surface SHALL support `vec![]` element and repeat forms.

#### Scenario: A vector is built from elements

- **WHEN** a program writes `vec![a, b, c]`
- **THEN** the result is a `Vec<T>` containing those elements in order

#### Scenario: A vector is built by repetition

- **WHEN** a program writes `vec![value; count]` with a non-negative `count`
- **THEN** the result contains `count` copies of `value`

#### Scenario: The form is not a general macro facility

- **WHEN** a program attempts to define its own `name![..]` form
- **THEN** the program is rejected, because `vec!` is a pinned built-in rather
  than a user-definable macro
