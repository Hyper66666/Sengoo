## ADDED Requirements

### Requirement: SQLite connection context SHALL be owned and isolated

`SqliteDb.set_context` SHALL copy bounded UTF-8 metadata into connection-local
storage. `sengoo_context(key)` SHALL return its value or SQL NULL without a SQL
mutation interface. Active transactions SHALL reject metadata mutation. Final
connection cleanup SHALL free context even when statements retain the connection.

#### Scenario: Trigger validates connection metadata
- **WHEN** a trigger reads a matching connection value through `sengoo_context`
- **THEN** the guarded write succeeds
- **AND** changing that value through the native setter changes subsequent checks
- **AND** another connection and a restored backup have no inherited context

#### Scenario: Bounded metadata
- **WHEN** a key exceeds 128 UTF-8 bytes, a value exceeds 4096, either contains NUL, or a 65th key is inserted
- **THEN** the setter fails through the stable status channel
- **AND** previously stored values remain intact

### Requirement: SQLite metadata and backup SHALL preserve native semantics

The package SHALL expose signed i64 change counts and last inserted rowids
through the existing Result/status channel. Synchronous backup SHALL use
SQLite's backup API for a persistent destination different from its source.
Connection operations SHALL continue to obey transaction ownership. Backup
failures SHALL preserve a SQLite code/message and SHALL NOT claim atomic archive
publication.

#### Scenario: Persistent backup
- **WHEN** a committed database is copied to a different file
- **THEN** reopening the destination observes the committed rows
- **AND** connection context is absent

#### Scenario: Full i64 identity
- **WHEN** an insert uses an i64 rowid beyond exact f64 precision
- **THEN** last_insert_rowid returns that exact integer and changes returns one
