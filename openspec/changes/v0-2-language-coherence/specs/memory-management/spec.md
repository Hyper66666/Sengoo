## ADDED Requirements

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
