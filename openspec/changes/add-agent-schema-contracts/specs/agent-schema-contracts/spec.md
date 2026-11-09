## ADDED Requirements

### Requirement: Tool schemas use explicit supported semantics
Schema validation SHALL reject unsupported or malformed definitions before
validating any instance, returning a stage, keyword and JSON Pointer.

#### Scenario: Optional unsupported rule
- **WHEN** an absent property declares an unsupported schema keyword
- **THEN** validation rejects the schema instead of silently accepting the instance

#### Scenario: Exact bounded validation
- **WHEN** Unicode strings, large integers or work limits are encountered
- **THEN** scalar lengths and integer values retain their exact documented semantics
- **AND** capacity exhaustion rejects without reporting partial validation as success
