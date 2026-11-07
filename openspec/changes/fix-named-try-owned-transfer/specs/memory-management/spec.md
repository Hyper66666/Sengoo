## ADDED Requirements

### Requirement: Named question-mark operands transfer ownership
Question-mark extraction from an owning named Result or Option SHALL transfer
the active payload and prevent cleanup of that payload through the old wrapper.

#### Scenario: Named JSON result survives function return
- **WHEN** a named successful Result owns a JSON document, is unwrapped with question-mark and its document is returned
- **THEN** the caller can read the document until its own owner is dropped
- **AND** native document accounting returns to its baseline afterwards

#### Scenario: Failure propagation preserves the error owner
- **WHEN** a named failing Result is propagated to its caller
- **THEN** the caller owns the active error and the source wrapper does not release it
- **AND** unrelated live locals are released exactly once

#### Scenario: Consumed owning operand cannot be reused
- **WHEN** code reads an owning wrapper after question-mark consumed it
- **THEN** compilation rejects the read with the existing move diagnostic
