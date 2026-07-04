## ADDED Requirements

### Requirement: Dropping a `dyn` value SHALL run the concrete `Drop`

Trait-object vtables SHALL carry a `drop` slot and size/align metadata, and
dropping a `dyn Trait` value SHALL invoke the concrete type's `Drop` exactly
once through the vtable.

#### Scenario: dyn value drops its concrete resource

- **WHEN** a `dyn Trait` value backed by a concrete type with `impl Drop`
  goes out of scope
- **THEN** the concrete `drop` runs exactly once through the vtable slot
- **AND** a concrete type without `impl Drop` produces no drop call

### Requirement: dyn dispatch SHALL cover `&mut self` and fail stably elsewhere

Dynamic dispatch SHALL support `&mut self` receivers, and still-unsupported
trait-object forms SHALL produce stable diagnostics instead of internal
errors.

#### Scenario: Mutable receiver dispatches through the fat pointer

- **WHEN** a program calls a `&mut self` trait method on a `&mut dyn Trait`
- **THEN** the call dispatches to the concrete impl and mutations are visible
  through the original value

#### Scenario: Unsupported dyn forms are stable diagnostics

- **WHEN** a program writes `dyn A + B` or `Box<dyn Trait>`
- **THEN** compilation fails with the documented stable code for that form

### Requirement: Hashing SHALL be an object protocol like formatting

The stdlib SHALL provide a `Hasher` type, and `impl Hash` SHALL be able to
define `hash_into(&self, h: &mut Hasher)` with a compiler-synthesized
`hash()` bridge.

#### Scenario: Custom hash_into satisfies Hash bounds

- **WHEN** a type implements `Hash` with only `hash_into`
- **THEN** generic code bounded by `Hash` can call `hash()` and receives the
  value produced by driving `hash_into` through a fresh `Hasher`

#### Scenario: Derived hash uses the same state

- **WHEN** a struct derives `Hash`
- **THEN** its hash equals feeding its fields in declaration order into the
  runtime hash state

### Requirement: Borrowed views SHALL be tracked through reassignment

The borrow checker SHALL follow `&str` views through local reassignment
chains so aliases neither escape the owner scope nor allow moving the owner.

#### Scenario: Rebound view cannot escape

- **WHEN** a view of an owned `String` is rebound (`let b = a;`) and the
  alias is returned or stored beyond the owner scope
- **THEN** compilation fails with `borrow-escapes-scope`

#### Scenario: Rebound view still pins the owner

- **WHEN** an alias of a live view exists and the program moves the owner
- **THEN** compilation fails with `cannot-move-borrowed`
