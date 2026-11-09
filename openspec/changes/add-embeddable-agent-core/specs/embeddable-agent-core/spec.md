## ADDED Requirements

### Requirement: Shared host-independent execution core
The sgagent package SHALL provide reusable policy, scheduling and action/event
execution without holding a CLI, credential catalog, terminal or private store.

#### Scenario: Independent host
- **WHEN** a native application registers model and tool adapters
- **THEN** it can drive the same core through read, modification, verification and resume
- **AND** extracting pure policy helpers alone does not count as completion

### Requirement: Durable dispatch and replaceable ledgers
Every model/tool dispatch and terminal outcome SHALL be gated by a committed
checkpoint with matching run, fence and monotonically increasing revision.

#### Scenario: Delayed or conflicting acknowledgment
- **WHEN** a receipt is stale or a duplicate revision has different content
- **THEN** it cannot advance the kernel or authorize a side effect

#### Scenario: Real persistence
- **WHEN** the reference SQLite host commits and restarts
- **THEN** the checkpoint, accounting and tool outcomes survive atomically
- **AND** an in-memory mock is not accepted as evidence of persistence

### Requirement: Permissions and recovery preserve intent
The core SHALL validate dynamic tool schema and captured permissions before
dispatch and shall preserve cumulative accounting and stable identities.

#### Scenario: Unknown tool outcome
- **WHEN** a claimed tool has no known result or cleanup is unconfirmed
- **THEN** execution pauses without replaying it or releasing its authority

#### Scenario: Plan and retry
- **WHEN** a Plan turn completes, pauses or is cancelled
- **THEN** the Plan overlay ends while historical policy remains recorded
- **AND** model retries retain completed tools and never reset cumulative budgets

#### Scenario: Corrupt recovery authority
- **WHEN** a checkpoint has malformed original policy, widened current authority or inconsistent cleanup/claim phases
- **THEN** recovery rejects it before dispatch or mutation
- **AND** normal turn-scoped Plan completion is preserved
