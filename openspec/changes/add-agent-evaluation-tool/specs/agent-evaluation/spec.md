## ADDED Requirements

### Requirement: Bounded redacted event recording
The evaluation tool SHALL validate and redact complete UTF-8 event JSONL before
writing versioned contiguous-sequence records.

#### Scenario: Secrets in nested fields and text
- **WHEN** events contain credential fields or explicitly configured literal secrets
- **THEN** those fields/strings are redacted recursively before output is written
- **AND** Unicode and exact integers remain intact

#### Scenario: Malformed input
- **WHEN** input violates JSON/UTF-8/schema/size/depth contracts
- **THEN** recording fails without creating partial successful output
- **AND** diagnostics do not echo raw input or secret values

#### Scenario: Embedded NUL before or after secret material
- **WHEN** raw input or decoded JSON keys, values, redactions or assertions contain NUL
- **THEN** exact owned bytes are checked before any C-compatible string view
- **AND** recording/replay fails without partial output, truncated comparisons or false success

### Requirement: Deterministic replay without execution
Replay SHALL evaluate recorded data without executing any recorded operation.

#### Scenario: Tool command in a recording
- **WHEN** an event contains command arguments with a file-writing side effect
- **THEN** replay only evaluates assertions and creates its explicit report
- **AND** the recorded command is never executed

#### Scenario: Validation and failure reports
- **WHEN** schema, sequence, assertion shape or output ownership is invalid
- **THEN** replay exits nonzero without a success report or overwriting existing output
- **AND** a valid but failing assertion produces a deterministic failed report
