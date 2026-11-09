# Add a durable SQLite Agent ledger

## Why
An in-memory commit receipt cannot establish durability, execution ownership or
recovery correctness. The embeddable Agent kernel needs a real store that a
non-TUI host can consume without CliState, Terminal, credentials or desktop schema.

## Scope
Add an experimental Sengoo SQLite adapter with canonical per-run OS locks,
monotonic database fencing, exact revisions and atomic checkpoint/receipt writes.
The store uses the existing sgsqlite package and generic FileLock/hash APIs.
Independent real-database tests cover conflicts, rollback, corruption, crash
admission, resource cleanup and unknown tool outcomes. No native implementation
or implicit tool execution is introduced.

## Boundaries
A lock proves exclusive admission, not process-tree quiescence. Host-confirmed
quiescence is an explicit trusted assertion; no heartbeat takeover is allowed.
The adapter is a dedicated local database, not a second writer to Sencode's
shared desktop storage. Read-only history viewing is not claimed because the
current sgsqlite API opens read/write and may create a file.
