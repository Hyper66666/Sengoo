## ADDED Requirements

### Requirement: The standard library SHALL provide generic owning collections

`std::collections` SHALL provide `Vec<T>`, `HashMap<K, V>`, `HashSet<T>`,
`BTreeMap<K, V>`, `BTreeSet<T>`, and `VecDeque<T>` that store arbitrary element
types, own their elements, and drop them automatically.

#### Scenario: Vec stores non-scalar elements

- **WHEN** a program creates a `Vec<String>` (or a `Vec` of a struct), pushes
  elements, and reads them back
- **THEN** the elements are stored and retrievable without scalar
  hand-specialization
- **AND** dropping the `Vec` drops every contained element with no leak

#### Scenario: Maps key on Hash/Eq or Ord

- **WHEN** a program inserts into a `HashMap<K, V>` keyed by a `Hash + Eq` type
  and into a `BTreeMap<K, V>` keyed by an `Ord` type
- **THEN** lookups return the inserted values
- **AND** `BTreeMap` iteration is in key order

#### Scenario: Ownership transfer on insert and remove

- **WHEN** a value is inserted into a collection and later removed
- **THEN** insertion moves the value in, reads borrow or clone it, and removal
  moves it back out to the caller

### Requirement: Iterator adapters SHALL be available over collections

The standard library SHALL provide `map`, `filter`, `fold`, `enumerate`, `take`,
`skip`, `count`, `sum`, and `collect` over the `Iterator` trait.

#### Scenario: Adapter chain collected into a Vec

- **WHEN** a program iterates a collection through `map` and `filter` and calls
  `collect`
- **THEN** the result is a new collection containing the transformed, filtered
  elements

### Requirement: Existing scalar collection helpers SHALL remain source-compatible

The previous scalar helpers SHALL continue to compile as wrappers over the
generic collections during the transition.

#### Scenario: Legacy scalar helper still compiles

- **WHEN** existing code calls `vec_new_i64` or uses `StringMapI64`
- **THEN** it continues to compile and behave as before
