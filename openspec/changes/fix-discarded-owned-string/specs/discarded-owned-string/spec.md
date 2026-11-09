## ADDED Requirements

### Requirement: Ignored owned loop tails SHALL be cleaned up

Non-returning body tails SHALL use the same owning complete-expression cleanup
as discarded expression statements. Borrowed variable/field aliases SHALL NOT
become duplicate owners.

#### Scenario: Loop tail propagates an owned String

- **WHEN** each iteration ends in a successful Result<String, i64> propagation
  whose String value is discarded
- **THEN** that String is released exactly once before the next iteration
- **AND** failed propagation cleans up active prior bindings before returning

### Requirement: Proven legacy observation parameters SHALL retain caller ownership

Concrete available HIR definitions SHALL establish conservative parameter
ownership metadata for legacy observation APIs. All argument lowering layers
SHALL honor the same policy. Observation SHALL NOT be inferred solely because
the function returns a scalar or lacks an explicit destructor.

#### Scenario: Named and temporary observed Strings

- **WHEN** String.eq observes named or freshly constructed String arguments
- **THEN** the caller retains ownership through the complete call
- **AND** both named input values remain readable afterwards
- **AND** each owner is released once at the appropriate cleanup boundary

#### Scenario: Borrowed Buffer passes through wrappers

- **WHEN** a stdlib hash wrapper forwards a Buffer to a proven observing
  parameter of another wrapper
- **THEN** the fixed-point summary preserves caller ownership across both calls
- **AND** the output Buffer remains usable after hashing and releases on exit

#### Scenario: Unknown or escaping ownership remains conservative

- **WHEN** a parameter is returned, stored in an owning aggregate, explicitly
  released, captured or passed to an unknown consuming position
- **THEN** it is not newly inferred to borrow
- **AND** unknown recursive cycles retain existing transfer behavior
- **AND** unsupported owned extern signatures remain rejected

#### Scenario: Failure after an observed call

- **WHEN** an observed call succeeds and a subsequent propagation returns early
- **THEN** the caller's String and Buffer owners are released exactly once
- **AND** no manual resource-release workaround is required by the consumer

### Requirement: Explicit references to fresh owners SHALL retain cleanup

Taking an explicit reference to a freshly produced owning value SHALL preserve
its owner through the call and register its cleanup. The source unary reference
form and generated HIR reference form SHALL use the same ownership rule. Named
references SHALL NOT acquire an additional owner or destructor call.

#### Scenario: Function and method borrow a fresh constructor result

- **WHEN** a function or method receives `&make_owner()` without a user binding
- **THEN** the referent remains valid during the borrowed call
- **AND** its owner is destroyed exactly once after its last supported use
- **AND** a later argument's failing propagation also cleans up that owner

#### Scenario: Repeated fresh reference calls release their resources

- **WHEN** a loop borrows fresh String, Buffer, JsonDoc and Vec values
- **THEN** all resource counts return to their baseline after the loop scope
- **AND** borrowing a named value does not destroy it before subsequent use
