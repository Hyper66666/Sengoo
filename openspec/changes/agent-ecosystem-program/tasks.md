## Publication and baseline (integration owner)
- [x] Publish verified CLI fixes and roadmap using the user's configured identity.
- [x] Record both committed baselines and assign non-overlapping owners.
- [x] Create a concrete requirement/evidence ledger for all roadmap deliverables.
- [x] Record current compile/startup/memory baselines and language correctness blockers.
- [x] Record the first version-process/build timing baseline, with its warm-cache scope.

## SDK integration (child owners; integration owner verifies consumers)
- [x] Complete sgllm three-protocol SDK and CLI migration to the same implementation.
- [x] Complete sgmcp client/server SDK and CLI migration to the same implementation.
- [x] Complete tool Schema subset validation with explicit unsupported diagnostics.
- [x] Provide generic pollable stdout and verify server cancellation under output backpressure.
- [x] Extract sgagent execution core, policy/budgets, durable state and tool-recovery boundary.
- [x] Complete sgeval offline recording/redaction/replay/assertion tool.

## Consumers and delivery (integration owner)
- [x] Repository audit consumer using two connections and a read-only tool report.
- [x] Local knowledge MCP server with directory policy and SQLite index; independent client.
- [x] Recoverable maintenance Agent that edits/tests/checkpoints and blocks unknown effects.
- [x] Locked offline package archives and checksums; consumers outside source checkouts.
- [x] Independent Windows programs and Python integration for supported native functions.
- [x] Versioned quick reference, checked tutorials, compatibility catalog and language examples.
- [x] Measured cold/warm compile, startup, RSS, long streams and cancellation/resource recovery.
- [x] Validate supported platforms individually; record deferred or unrun platforms honestly.

## Further roadmap scope
- [ ] Track and implement child agents, cross-provider consultation, workflows, memory and remote MCP from actual consumer contracts and executable checks.
- [x] Final requirement-by-requirement audit, with remaining limitations and publication scope explicit.

Windows development evidence: docs/agent-ecosystem-acceptance.md and target/agent-completion-v2/acceptance-index.json. Platform qualification here records Windows passes and other-host deferrals; it does not assert their success. Public release and independent trials remain separate.
