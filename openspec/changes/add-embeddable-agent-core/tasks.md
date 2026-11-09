## Core owner
- [x] Extract policy/GoalState, scheduler, tool_schedule and request_retry with compatible symbols and no CLI dependency.
- [x] Add dynamic tool schemas and captured permission decisions.
- [x] Implement the host-independent action/event loop with durable commit/receipt dispatch gates.
- [x] Preserve request IDs, cumulative budgets and tool identities across snapshot recovery.
- [x] Pause cancellation, interrupted requests, unknown outcomes and unconfirmed cleanup without tool replay.
- [x] Pass pure/kernel/repeated-resource tests with the selected toolchain.
- [x] Deliver and verify an external no-TUI read/patch/test/resume consumer using locked packages.
- [x] Document APIs, state/storage contracts, examples and actual platform evidence.

## Integration owner
- [x] Implement and verify the real SQLite ledger, OS locks, fencing and atomic commit receipts.
- [x] Map Sencode to the same pure implementation and kernel invariants without changing authoritative shared-history contracts.
- [x] Pass existing CLI Plan/Goal/permissions/cancel/recovery gates and produce local development candidate packages; public release remains external.
