## ADDED Requirements

### Requirement: Sengoo SHALL support generic functions
The compiler SHALL accept generic function declarations with type parameters and instantiate them at call sites.

#### Scenario: Generic function call type-checks with inferred type
- **WHEN** a generic function is called without explicit type arguments and argument types are sufficient
- **THEN** the type checker infers type parameters and accepts the call

### Requirement: Sengoo SHALL support generic structs
The compiler SHALL accept generic struct declarations and enforce type-argument correctness at construction and field access.

#### Scenario: Generic struct construction validates type arguments
- **WHEN** a generic struct instance is created with concrete type arguments
- **THEN** field types are checked against the instantiated struct type
