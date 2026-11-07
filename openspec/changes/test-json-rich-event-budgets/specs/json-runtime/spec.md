## ADDED Requirements

### Requirement: Rich metadata explicit-budget regression
The native regression suite SHALL independently verify explicit structural JSON
budgets and cleanup while preserving the legacy JSON constructor behavior.

#### Scenario: Node-rich metadata and repeated cleanup
- **WHEN** metadata containing more than 4096 nodes is parsed twenty times with an explicit 65536-node budget
- **THEN** every field round trips and all document and reservation counters return to baseline
- **AND** the legacy parser retains error 10 and insufficient explicit capacity returns error 13

#### Scenario: Malformed event metadata
- **WHEN** syntactically invalid metadata is parsed with explicit limits
- **THEN** parsing fails with error 10 and retains no document reservation
