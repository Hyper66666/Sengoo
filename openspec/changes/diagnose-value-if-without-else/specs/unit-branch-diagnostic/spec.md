## ADDED Requirements

### Requirement: Unit branch mismatches identify their source
The compiler SHALL report a positioned `if-without-else-value` diagnostic when a
then branch producing a value cannot unify with an omitted else. The diagnostic
SHALL identify the enclosing function and suggest an explicit unit or else.

#### Scenario: Boolean call at the end of an omitted-else branch
- **WHEN** a branch ends in a boolean-returning call and has no else
- **THEN** type checking fails at that branch's last statement with the repair hint

#### Scenario: Explicit unit and conditional return remain valid
- **WHEN** a branch explicitly produces unit or returns from its function
- **THEN** the existing valid control flow continues to compile
