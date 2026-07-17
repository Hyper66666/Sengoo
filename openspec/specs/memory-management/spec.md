# memory-management Specification

## Purpose
TBD - created by archiving change automatic-memory-management. Update Purpose after archive.
## Requirements
### Requirement: Owning values SHALL be released automatically at end of scope

The compiler SHALL insert `Drop` glue so that any value owning a resource is
released when its owner goes out of scope, without the program calling a free or
close method.

#### Scenario: A heap value is dropped at scope end

- **WHEN** a function creates an owning value (a heap container, owned string, or
  runtime handle) and returns without calling any explicit release method
- **THEN** the compiler inserts a `drop` call at the end of the value's scope
- **AND** running the program under a leak-checking build reports no leak for
  that value

#### Scenario: Drop runs on early exit and propagation

- **WHEN** a scope exits early via `return`, `?` propagation, `break`, or
  `continue` while an owning local is still live
- **THEN** the live owning locals are dropped before control leaves the scope
- **AND** locals already moved out are not dropped again

#### Scenario: Drop order is reverse declaration order

- **WHEN** multiple owning locals in the same scope are dropped at scope end
- **THEN** they are dropped in reverse order of declaration

### Requirement: The `Drop` trait SHALL define automatic cleanup

A type SHALL be able to implement `Drop` with a single `def drop(&mut self)`
method that the compiler calls automatically and that user code SHALL NOT call
directly except through the compatibility release methods.

#### Scenario: User type with Drop is finalized

- **WHEN** a value of a user type that implements `Drop` goes out of scope
- **THEN** the compiler calls its `drop` method exactly once
- **AND** a direct user call to `drop` is rejected at type-check time

### Requirement: Use-after-move SHALL be a compile error

The type checker SHALL treat a non-`Copy` value as moved when it is passed by
value, returned, or assigned, and SHALL reject any later read of the moved-from
binding with a stable diagnostic.

#### Scenario: Reading a moved value is rejected

- **WHEN** a non-`Copy` value is moved and then read again on a reachable path
- **THEN** type-check fails with the stable `use-after-move` diagnostic code
- **AND** the same stable code appears in `sgc --error-format json` output and in
  the `sglsp` diagnostic

#### Scenario: Copy values are not moved

- **WHEN** a `Copy` scalar (integer, float, bool) or a reference `&T` is used
  after being passed or assigned
- **THEN** the later use compiles without a move error

### Requirement: Explicit release SHALL remain source-compatible and double-free-safe

Existing `free()` / `drop()` / `close()` methods SHALL continue to compile, and
combining an explicit release with automatic drop SHALL NOT cause a double free.

#### Scenario: Explicit release followed by scope end

- **WHEN** a program calls an explicit release method on an owning value and then
  the value goes out of scope
- **THEN** the explicit call releases the resource and marks the value moved
- **AND** no automatic drop runs for that value at scope end
- **AND** the previously committed manual-release examples still compile and run

### Requirement: Shared ownership SHALL be opt-in via `Rc<T>`

The standard library SHALL provide an opt-in `Rc<T>` shared-ownership type rather
than making reference counting the default.

#### Scenario: Shared ownership through Rc

- **WHEN** a value is wrapped in `Rc<T>` and cloned
- **THEN** the underlying value is released only after the last `Rc` handle is
  dropped
- **AND** the default (non-`Rc`) types remain move-only with single ownership

#### Scenario: User-defined owning payload is shared through Rc

- **WHEN** a user-defined aggregate containing a `Drop` field is moved into
  `Rc<T>`, borrowed through one clone, and all clones leave scope
- **THEN** every clone observes the same payload address and value
- **AND** the aggregate drop glue runs exactly once after the final clone
- **AND** no payload or control-block handle remains live

#### Scenario: Rc borrow keeps its owner live

- **WHEN** a program borrows `&T` from an `Rc<T>` and attempts to move or drop
  the last owning `Rc<T>` before the borrow ends
- **THEN** type checking rejects the move with the existing
  `cannot-move-borrowed` diagnostic

### Requirement: Local borrows SHALL end at their last reachable use

The compiler SHALL infer intraprocedural borrow regions so a borrow no longer
blocks owner mutation or move after its last reachable use, while conservatively
keeping the borrow live across control-flow paths that may use it.

#### Scenario: Owner moves after last borrow use

- **WHEN** a local reference is used for the last time and its owner is moved on
  every subsequent reachable path
- **THEN** the move compiles
- **AND** no generated code reads through the expired reference

#### Scenario: One branch still uses the borrow

- **WHEN** a branch can use a reference after an attempted owner move
- **THEN** type checking fails with `cannot-move-borrowed`

### Requirement: Borrowed results SHALL not outlive their source owner

A returned reference SHALL be derived from an input reference whose lifetime
covers the result and SHALL NOT point into a local, temporary, or by-value owner.

#### Scenario: Function returns a local reference

- **WHEN** a function returns a reference to a local or temporary value
- **THEN** type checking fails with `borrow-escapes-owner`

#### Scenario: Function returns a projection of an input reference

- **WHEN** a function returns a field or element reference derived from an input
  reference and does not move the input owner
- **THEN** the function compiles and the result remains tied to that input

### Requirement: Named aggregate partial moves SHALL be exact

Moving an owning named field SHALL invalidate that field without invalidating
independent fields, and Drop SHALL run only for initialized, unmoved paths.

#### Scenario: One field is moved out

- **WHEN** a non-Copy field is moved from a struct and another independent field
  is read before scope exit
- **THEN** the independent read succeeds
- **AND** reading the moved field or whole struct fails with
  `use-after-partial-move`
- **AND** scope-exit Drop skips the moved field and drops remaining owning fields
  exactly once

### Requirement: Owning temporaries and arrays SHALL receive deterministic Drop

Unmoved owning temporaries SHALL be dropped at full-expression end, and owning
fixed arrays SHALL drop initialized elements in reverse index order.

#### Scenario: Temporary is not retained

- **WHEN** an owning call result is consumed only within one full expression
- **THEN** it is dropped exactly once after that expression

#### Scenario: Array leaves scope after an element initialization failure path

- **WHEN** only a prefix of an owning array is initialized before structured
  control leaves the scope
- **THEN** only initialized elements are dropped
- **AND** they are dropped in reverse index order
