## ADDED Requirements

### Requirement: Trusted-root bounded local knowledge
The independent Sengoo consumer SHALL expose registered MCP indexing, search,
status and read tools under a host-selected local directory without model root overrides.

#### Scenario: File and protocol boundaries
- **WHEN** a client supplies invalid types, extra root arguments, traversal, absolute paths, NUL or reparse traversal
- **THEN** schema or file validation rejects the request without returning outside-root bytes
- **AND** valid Unicode text and its digest remain intact

#### Scenario: Unsupported or oversized text
- **WHEN** an index candidate contains invalid UTF-8, NUL or exceeds the per-file byte limit
- **THEN** indexing counts a skipped file and current-file read reports a tool error
- **AND** no truncated text is represented as the complete file

### Requirement: Persistent atomic knowledge index
The consumer SHALL retain its committed SQLite index across restarts and refresh only on explicit request.

#### Scenario: Restart and deleted source
- **WHEN** a source file is deleted after a successful index and the server restarts
- **THEN** status/search retain the committed generation and indexed text while current read reports missing file
- **AND** explicit reindex removes the obsolete stored entry

#### Scenario: Refresh write failure
- **WHEN** SQLite rejects an insert after the refresh deletes its old rows
- **THEN** the transaction rolls back and status/search retain the prior complete index and generation
