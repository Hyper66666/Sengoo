# generic-collections Specification

## Purpose
TBD - created by archiving change generic-collections. Update Purpose after archive.
## Requirements
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

#### Scenario: Generic storage preserves layout and exact Drop

- **WHEN** a collection stores an over-aligned user struct containing owned
  fields and grows, replaces, removes, clears, or drops entries
- **THEN** each element remains correctly aligned and readable
- **AND** every still-owned value is dropped exactly once
- **AND** allocation or callback failure leaves the collection valid

#### Scenario: Mutation cannot invalidate a live element borrow

- **WHEN** code holds an element borrow and attempts a collection mutation that
  may move or reallocate storage
- **THEN** borrow checking rejects the mutation until the borrow ends

### Requirement: Iterator adapters SHALL be available over collections

The standard library SHALL provide `map`, `filter`, `fold`, `enumerate`, `take`,
`skip`, `count`, `sum`, and `collect` over the `Iterator` trait.

Generic `sum` SHALL preserve the iterator item type and SHALL only be available
when that type implements the numeric `SumValue` identity-and-combine contract.
An empty iterator SHALL return the stable identity for that numeric type.
Collection targets SHALL use explicit `collect_hashset` and
`collect_hashmap(projector)` sinks rather than relying on return-type-only
generic method inference.

#### Scenario: Adapter chain collected into a Vec

- **WHEN** a program iterates a collection through `map` and `filter` and calls
  `collect`
- **THEN** the result is a new collection containing the transformed, filtered
  elements

#### Scenario: Empty and chained numeric sums use the declared identity

- **WHEN** a numeric owning iterator is empty or is transformed through lazy
  adapters and then summed
- **THEN** the empty result is the numeric identity
- **AND** each yielded item is combined exactly once without an `i64` fallback

#### Scenario: Explicit set and map sinks preserve generic ownership

- **WHEN** an owning iterator collects into a hash set or projects items into
  `MapEntry<K,V>` values for a hash map
- **THEN** keys and values move into the generic collection core
- **AND** duplicate entries follow the target collection replacement semantics

### Requirement: Existing scalar collection helpers SHALL remain source-compatible

The previous scalar helpers SHALL continue to compile as wrappers over the
generic collections during the transition.

#### Scenario: Legacy scalar helper still compiles

- **WHEN** existing code calls `vec_new_i64` or uses `StringMapI64`
- **THEN** it continues to compile and behave as before
