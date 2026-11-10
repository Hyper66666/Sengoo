## ADDED Requirements

### Requirement: HIR6 SHALL distinguish unchecked trait default bodies from checked executable bodies

HIR6 SHALL preserve the complete AST, type arena, resolution, call types, scopes,
module witness and actual ownership without changing the wire schema or budgets.
A trait default body skipped by the actual semantic checker SHALL be represented
as deferred source syntax, not checked native job authority.

#### Scenario: The complete standard graph contains default bodies
- **WHEN** an actual trait declaration owns an actual function declaration and its unique body subtree has the semantic checker's unvisited metadata sentinels
- **THEN** HIR MAY retain that body without inventing callable facts for its unvisited calls
- **AND** every raw AST/type/reference bound SHALL still be checked
- **AND** executable job consumers SHALL reject that deferred body until independent concrete body checking is implemented

#### Scenario: Deferred coverage is forged
- **WHEN** the declaration, trait owner, unique syntactic parent/body membership or unvisited metadata is altered
- **THEN** HIR validation SHALL reject rather than treating an ordinary unresolved call as deferred
- **AND** shared or cross-attached deferred subtrees SHALL NOT acquire coverage

#### Scenario: An ordinary checked call lacks callable facts
- **WHEN** a call outside a proved deferred body has missing or incompatible callable type, result, arity or target facts
- **THEN** HIR validation SHALL continue to reject

### Requirement: HIR callable identity SHALL compare resolved structure without changing typed input

A saved callable and a callee node MAY use different interned Fn TypeIDs when
semantic substitutions make their structures identical. Validation SHALL compare
resolved kind, data, flags, nominal identity, arity and all type arguments under
explicit recursion/work limits, without interning or weakening type compatibility.

#### Scenario: Function-valued generic fields are normalized at semantic completion
- **WHEN** the callee Fn has the resolved generic return while the saved call Fn retains a substituted inference TypeID
- **THEN** validation SHALL accept only if every recursively resolved type component is identical
- **AND** serialization and cloning SHALL retain the original raw type IDs and all owned input tables unchanged

#### Scenario: Signature metadata changes
- **WHEN** ABI, flags, element/result type, nominal identity, arity or substitution relationships differ
- **THEN** the ordinary call SHALL be rejected
- **AND** validation SHALL NOT materialize new types to make the comparison pass

### Requirement: HIR module provenance and ownership SHALL remain mandatory

Structural deferred-body coverage SHALL NOT authorize provider use, constructor
allocation, runtime references, generic specialization or missing source coverage.
Actual module validation SHALL retain exact source/checked/ownership witnesses.

#### Scenario: A scoped constructor is clean within an otherwise incomplete library
- **WHEN** independently reconstructed scoped ownership is supplied for an actual entry function
- **THEN** full raw standard graph HIR MAY be stored and roundtripped
- **AND** complete reachable job coverage and the same fresh source/provider context SHALL remain required before native lowering

#### Scenario: Old or altered serialized data is supplied
- **WHEN** the wire schema is five, the module witness changes or the actual ownership differs
- **THEN** decoding or actual module validation SHALL reject
- **AND** schema6, all22 ownership fields and existing builtin6/deferred4/actions8/9/10 contracts SHALL remain unchanged
