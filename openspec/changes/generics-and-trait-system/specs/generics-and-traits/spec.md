## ADDED Requirements

### Requirement: Generic functions, methods, and types SHALL be monomorphized over arbitrary type parameters

The compiler SHALL accept generic `def`, `impl`, `struct`, and `enum` with type
parameters and SHALL produce one specialized implementation per concrete set of
type arguments.

#### Scenario: One generic function used at two types

- **WHEN** a generic function is called with two different concrete type
  arguments in the same program
- **THEN** both calls type-check and run with the correct per-type behavior
- **AND** the compiler emits a specialized body for each instantiation with a
  deterministic name

#### Scenario: Generic type with generic methods

- **WHEN** a generic `struct`/`enum` with a generic `impl` is instantiated at a
  concrete type
- **THEN** its methods are available and type-checked at that concrete type
- **AND** a user-defined generic `Result<T, E>` works without a hand-written
  concrete impl per type

### Requirement: Trait bounds SHALL be checked at use sites

A generic item SHALL be able to require `T: Trait` (directly or via `where`), and
the compiler SHALL reject instantiations whose type arguments do not satisfy the
bounds.

#### Scenario: Satisfied bound compiles

- **WHEN** a bounded generic is instantiated with a type that has the required
  `impl Trait for Type`
- **THEN** the program type-checks and the trait methods are callable on the
  bounded value

#### Scenario: Unsatisfied bound is rejected

- **WHEN** a bounded generic is instantiated with a type that lacks the required
  impl
- **THEN** type-check fails with the stable `unsatisfied-trait-bound` diagnostic
  naming the trait, the type, and the missing method(s)
- **AND** the same stable code is present in `sgc --error-format json` and
  `sglsp`

### Requirement: Trait objects SHALL provide dynamic dispatch

The language SHALL support `dyn Trait` values that dispatch trait methods through
a vtable, restricted to object-safe traits.

#### Scenario: Dynamic dispatch through a trait object

- **WHEN** a value is coerced to `dyn Trait` and a trait method is called on it
- **THEN** the call dispatches to the concrete type's implementation at runtime

#### Scenario: Non-object-safe trait rejected

- **WHEN** a program attempts to form `dyn Trait` for a trait that is not
  object-safe
- **THEN** type-check fails with the stable `not-object-safe` diagnostic
  explaining the violated rule

#### Scenario: Dropping a trait object runs the concrete Drop

- **WHEN** an owning `dyn Trait` value goes out of scope
- **THEN** the concrete type's `Drop` is invoked through the vtable drop slot

### Requirement: Traits SHALL support associated types

A trait SHALL be able to declare associated types that each impl defines, and
generic code SHALL refer to them as `T::AssocName`.

#### Scenario: Associated type resolved in generic code

- **WHEN** generic code bounded by a trait references the trait's associated type
- **THEN** the associated type resolves to the concrete impl's definition during
  monomorphization

#### Scenario: Trait object fixes associated types

- **WHEN** a trait with an associated type is used as a trait object
- **THEN** the object type SHALL fix the associated type (e.g. `dyn Iterator<Item = i64>`)
- **AND** an unfixed associated type in an object type is a stable diagnostic

### Requirement: A core trait set SHALL be available and derivable

The standard prelude SHALL define `Clone`, `Copy`, `PartialEq`/`Eq`,
`PartialOrd`/`Ord`, `Hash`, `Default`, `Display`, `Debug`, `Iterator`, and
`IntoIterator`, and `#[derive(...)]` SHALL generate impls for the derivable ones.

#### Scenario: Derive generates working impls

- **WHEN** a struct or enum has `#[derive(Clone, PartialEq, Debug)]`
- **THEN** the type can be cloned, compared with `==`, and debug-formatted
- **AND** the generated impls respect the field/variant structure

#### Scenario: Copy and Drop are mutually exclusive

- **WHEN** a type is declared `Copy` while also implementing `Drop`, or has a
  non-`Copy` field
- **THEN** type-check fails with a stable diagnostic

### Requirement: Impls SHALL obey a package-local orphan rule

An `impl Trait for Type` SHALL be permitted only when the trait or the type is
defined in the current package.

#### Scenario: Orphan impl rejected

- **WHEN** a package writes `impl Trait for Type` where both the trait and the
  type come from other packages
- **THEN** type-check fails with the stable orphan-rule diagnostic
