## ADDED Requirements

### Requirement: Struct literal field sets MUST be complete and valid
The type checker SHALL validate struct literal fields against the declared struct definition before accepting the expression.

Validation SHALL enforce all of the following:
- Every declared struct field MUST be present exactly once.
- Duplicate field names in the same literal MUST be rejected.
- Field names not declared on the target struct MUST be rejected.
- If one or more field-set violations exist, compilation MUST fail.

#### Scenario: Complete struct literal passes validation
- **WHEN** a struct `Point { x: i64, y: i64 }` is initialized as `Point { x: 1, y: 2 }`
- **THEN** type checking succeeds for field-set validation

#### Scenario: Missing required field is rejected
- **WHEN** a struct `Point { x: i64, y: i64 }` is initialized as `Point { x: 1 }`
- **THEN** type checking fails and the diagnostic mentions missing field `y`

#### Scenario: Duplicate field is rejected
- **WHEN** a struct `Point { x: i64, y: i64 }` is initialized with two `x` assignments
- **THEN** type checking fails and the diagnostic mentions duplicate field `x`

#### Scenario: Unknown field is rejected
- **WHEN** a struct `Point { x: i64, y: i64 }` is initialized with field `z`
- **THEN** type checking fails and the diagnostic mentions unknown field `z`

#### Scenario: Multiple field-set issues are reported together
- **WHEN** a struct literal has duplicate, unknown, and missing field issues in one expression
- **THEN** type checking fails with one actionable diagnostic that includes each non-empty category

### Requirement: Struct literal field-set diagnostics MUST be deterministic
For the same source program, struct literal field-set diagnostics SHALL produce stable field-name ordering so tests and tooling can rely on consistent output.

#### Scenario: Deterministic ordering for multiple names
- **WHEN** a struct literal error includes multiple missing, duplicate, or unknown field names
- **THEN** each category is reported in deterministic (lexicographically sorted) name order
