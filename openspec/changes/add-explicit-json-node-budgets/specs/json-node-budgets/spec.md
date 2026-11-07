## ADDED Requirements

### Requirement: Explicit structured document admission
The new APIs SHALL enforce explicit byte, node, link and aggregate limits
without changing the existing JSON constructors. Owner: integration agent.

#### Scenario: Rich document below the explicit budgets
- **WHEN** a valid document exceeds4096 nodes but fits its explicit budgets
- **THEN** parse, deep copy and serialization succeed and preserve every value
- **AND** legacy parsing still rejects that document at4096 nodes

#### Scenario: Invalid or exhausted budgets
- **WHEN** a limit is invalid or an input exceeds its byte budget
- **THEN** rejection occurs before reading its input pointer
- **AND** explicit resource exhaustion reports13 while invalid JSON reports10

#### Scenario: Ownership and rollback
- **WHEN** a deep copy exceeds its budget or a document is closed or dropped
- **THEN** failed copies restore charges and owned nodes atomically
- **AND** destruction releases the document's aggregate reservation
