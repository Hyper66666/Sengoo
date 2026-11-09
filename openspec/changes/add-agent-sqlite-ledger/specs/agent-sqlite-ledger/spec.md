## ADDED Requirements

### Requirement: OS admission and monotonic fencing
The SQLite adapter SHALL acquire a canonical per-run OS lock before atomically
incrementing its durable fence and SHALL validate that fence for every read/write.

#### Scenario: Two executors contend
- **WHEN** a second process opens the same physical database and run while the first owns its lock
- **THEN** the second fails without advancing its durable fence
- **AND** no heartbeat timeout grants execution authority

#### Scenario: Delayed old executor
- **WHEN** a durable fence is newer than the caller's captured token
- **THEN** its read or commit is refused with status 12 regardless of cached ownership

### Requirement: Exact durable commit receipts
The adapter SHALL return a receipt only after snapshot, digest and receipt have
committed in one SQLite transaction under the exact v1 action identity.

#### Scenario: Sequential and repeated commits
- **WHEN** a new revision follows the saved revision or the current revision repeats
- **THEN** only saved+1 is a new write and only byte-identical current snapshots receive an idempotent acknowledgement
- **AND** stale, skipped, conflicting or inner/outer identity mismatches fail with status 12

#### Scenario: Failure between checkpoint and receipt writes
- **WHEN** the receipt write fails after the head has been updated
- **THEN** the transaction rolls back and load returns the prior complete snapshot
- **AND** no success receipt is returned

#### Scenario: Corrupt or oversized stored data
- **WHEN** metadata version, digest, receipt, identity or explicit capacity is invalid
- **THEN** reading or committing fails explicitly without truncating data, resetting budgets or replacing the checkpoint

### Requirement: Unknown effects survive loss of admission
A released OS lock SHALL NOT imply that prior tools are quiescent or that an
unknown side effect can be dispatched again.

#### Scenario: Host exits after a tool claim
- **WHEN** a persisted claim requires cleanup and its owning process exits
- **THEN** ordinary open remains refused until the trusted host explicitly confirms old executor/tool-tree quiescence
- **AND** kernel recovery retains the claim as paused with unknown outcome instead of replaying the tool

#### Scenario: Close with pending cleanup
- **WHEN** a live store has a cleanup-required checkpoint
- **THEN** explicit close fails and keeps the lock held
- **AND** the persistent guard remains after Drop or process exit releases the OS lock
