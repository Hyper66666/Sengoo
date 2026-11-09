## SQLite adapter owner
- [x] Align exact action/receipt/schema and quiescence contracts with the Agent core owner.
- [x] Implement owned SQLite store, canonical per-run OS lock and transactional monotonic fencing.
- [x] Implement bounded exact snapshot validation, digest/receipt atomic commit and checked load.
- [x] Reject stale/conflicting/skipped revisions, NUL, metadata mismatch and cumulative counter reset.
- [x] Preserve cleanup-required state and require explicit trusted-host quiescence for uncertain takeover.
- [x] Pass real SQLite rollback, corruption, revision, fencing, budget and resource-count regressions.
- [x] Pass independent-process lock contention, crashed-host cleanup guard and no tool replay.
- [x] Pass formatter and strict OpenSpec checks; record candidate/source/compiler hashes and remaining limits.
