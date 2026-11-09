# Embeddable Agent core

## Why
Sencode's policy, scheduling, request-retry and tool-loop invariants should be
usable by independent native applications without its TUI, connection catalog
or private session schema. The model and MCP SDKs now supply the reusable I/O.

## Capabilities
Add experimental sgagent 0.1.0: preserved pure policy/GoalState, scheduler,
tool scheduling and retry APIs plus a host-independent action/event kernel.
Dynamic tools use declared schema and captured policy. A replaceable ledger
commit/receipt boundary gates model dispatch, tool claims/results and terminal
checkpoints; a real SQLite adapter and external maintenance consumer prove it.

## Non-Goals
This change does not replace the CLI with a second copied controller, alter its
shared database format, add a global credential store, impersonate provider
success, or claim language features outside the documented ownership subset.
Existing Plan/Goal/subagent workflows must retain their individual integration
gates; extracting policy types alone does not complete this change.

## Ownership
The core owner owns the kernel, compatibility pure modules, dynamic registry,
core tests and examples. The integration owner exclusively owns
`packages/sgagent/src/sqlite_store.sg`, `tests/sqlite_store.sg`, CLI adapters,
shared-store compatibility and final combined/distribution verification.
