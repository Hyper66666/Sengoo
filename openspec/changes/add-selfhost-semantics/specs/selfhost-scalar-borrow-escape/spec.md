## ADDED Requirements

### Requirement: Returning a copied primitive read through a valid borrow SHALL NOT be a reference escape

For the path-escape check, checked bool, integer, floating-point and character
read values SHALL be distinguished from the borrow or owning aggregate through
which they were obtained. Existing use/move/loan/drop and legacy-handle checks
SHALL execute independently before any return escape decision.

#### Scenario: A Some payload points to a non-Copy owner
- **WHEN** valid source borrows an Owner through Option<&Owner>, guards Some and returns the Owner's primitive field value
- **THEN** the copied primitive result SHALL NOT be rejected solely because the root Option binding is a loan holder
- **AND** the Owner SHALL retain its existing exactly-once cleanup and move restrictions

#### Scenario: Primitive field and dereference values are returned
- **WHEN** a valid local borrow is read as a checked kind1/2/3/4 bool/int/float/char value and that value is returned directly, explicitly or through a branch
- **THEN** its scalar read SHALL NOT be treated as returning a local reference
- **AND** this rule SHALL NOT depend on a type's byte width or owning-handle name

### Requirement: Nonprimitive escape and separate provider authority SHALL remain unchanged

The scalar exception SHALL apply only to those four checked primitive kinds in
the existing path branch. Ref, raw pointer, unresolved generic, ADT, array and
tuple values SHALL retain their existing escape checks. It SHALL NOT be a
blanket exemption for every type that does not carry a borrow.

#### Scenario: A local reference or borrowed aggregate is returned
- **WHEN** source returns a reference or reference-containing aggregate derived from a local owner
- **THEN** ownership SHALL continue to reject the escape
- **AND** moving a non-Copy Owner through a borrow or invalidating a still-live owner SHALL retain the existing diagnostic

#### Scenario: A runtime optional borrow lacks source or branch authority
- **WHEN** a provider-derived pointer, stale backing or false-Some access fails its independent source/lifetime/Some proof
- **THEN** this scalar return rule SHALL NOT grant provider or native reference authority
- **AND** its independent rejection SHALL remain effective

### Requirement: The correction SHALL preserve serialized ownership and resource behavior

ShOwnership22, HIR6, builtin6/deferred4 tables and actions8/9/10 SHALL retain their
existing meanings. The checker SHALL produce its actual new results without
clearing previous errors or diagnostic arrays. Existing source-observation,
canonical disarm, HIR ownership and native resource regressions SHALL remain
qualified against the captured implementation and selected tools.

#### Scenario: A primitive field result survives owner cleanup
- **WHEN** an owning fixture returns the read scalar and its scope ends
- **THEN** actual execution SHALL preserve that scalar value and release the Owner exactly once
- **AND** references and owning values SHALL NOT become Copy by this rule
