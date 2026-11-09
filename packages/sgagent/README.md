# sgagent 0.1.0 — experimental embeddable Agent core

The action core and reference SQLite adapter have executable Windows evidence
in [EVIDENCE.md](EVIDENCE.md). Full Sencode integration, distribution and the
remaining Agent ecosystem roadmap have separate acceptance gates.
Requires Sengoo 0.2.1-dev.0 with the selected repaired ownership compiler,
edition 2026, native backend. Dependencies are `sgschema` and `sgsqlite`.

The package owns Agent state and dispatch decisions, not networking, credentials,
a UI, shell execution or a terminal. Hosts consume JSON actions and return
completed model turns, independent approvals, tool results and durable ledger
receipts. Use `sgllm` for model requests and `sgmcp` when implementing MCP tools.
The kernel and store are single-owner, non-Send/non-Sync objects.

## Action loop

`agent_kernel_new(run_id, fence, target, history, tools, policy, maximum_requests,
token_budget)` captures the model/connection target, tool registry, user history
and permissions. `maximum_requests = 0` has no request-count limit. Token budgets
are explicit and require reported exact usage; missing usage pauses safely.

`agent_kernel_next` returns an owning document with a stable action ID:

| Action | Host responsibility |
| --- | --- |
| `persist` | Atomically commit its complete checkpoint, then return matching receipt |
| `model_request` | Dispatch once using captured target/history/tools; accept only a complete validated canonical turn |
| `approval` | Obtain an independent permission decision and return matching call ID |
| `tool_request` | Execute the durably claimed call once; prove the process/resource tree quiescent before returning its known result |
| `retry_wait` | Poll cancellation and deadline; call retry_ready when due; never sleep inside the kernel |
| `complete` | Display the accepted result; no further work is authorized |
| `paused` | Preserve reason and cleanup requirements; no automatic side-effect replay |

Polling does not dispatch work. A host must not execute a repeated action ID
twice. Before model dispatch the request count is committed; before tool dispatch
the claim and cleanup ownership are committed. `agent_kernel_accept_model`
deduplicates request IDs, refuses reused tool IDs and preserves complete call/
result pairs. The kernel validates registered parameter schemas before tool
dispatch. Dynamic schema definitions use the supported `sgschema` subset and
unknown schema keywords fail instead of being silently ignored.
An explicit provider continuation keeps the same task without inventing a user
message. Complete HTTP/provider validation is still the model adapter's duty;
an incomplete stream cannot authorize tools by setting a flag itself.

Models cannot modify the registry or policy through their response. Tool registry
objects contain `name`, `description`, `parameters` and optional `read_only`,
`process`, `exclusive`, `plan_safe` flags. These declarations are host-provided
trust, not a system sandbox. Plan only admits reads and rejects process/write
tools; manual mode asks before side effects; auto-edit does not automatically
grant commands; full access is an explicit host capture. Plan ends on every
complete/pause/cancel, while `initial_policy` remains historical evidence.

The original pure `policy_*` / `goal_*`, `scheduler_*`, `tool_schedule_*` and
`request_retry` APIs remain for Sencode's thin adapters. A dynamic registry and
action kernel are additional APIs, not a second hardcoded CLI controller.

## Ledger boundary

`AgentCommitReceipt { run_id, fence, revision }` is returned only after a host's
durable commit. A `persist` action contains `schema`, `type`, `id`, `run_id`,
`fence`, `revision` and `checkpoint`. New revisions are exactly previous + 1.
Repeating a current revision is idempotent only for a byte-identical checkpoint;
stale, skipped, conflicting or fenced-out writes fail. Kernel acknowledgment
must exactly match the current proposed state. No I/O action is available while
that state remains unacknowledged.

`agent_memory_ledger/commit/snapshot` is a reference in-process adapter for
deterministic tests. It is not crash-durable storage. The real adapter exports
`agent_sqlite_open`, `agent_sqlite_commit`, `agent_sqlite_load` and explicit
quiescence-gated takeover. It uses an OS execution lock, monotonic fencing,
SQLite transactions and checksummed snapshots. See the authoritative storage
implementation and its tests for exact compatibility/version gates.

`agent_sqlite_open(&String path, &String run_id, maximum_snapshot_bytes)` accepts
limits from 1 to 4,194,304 bytes and returns an owning `AgentSqliteStore`.
`agent_sqlite_open_after_quiescence` additionally requires the host's explicit
`confirmed_previous_quiescent` assertion. `agent_sqlite_commit(&mut store,
&action)` returns a receipt only after its transaction commits;
`agent_sqlite_load(&store)` returns an independent owning checkpoint document.
`store.close()` refuses an unresolved cleanup claim or phase 5 and retains the
execution lock. Visible database and lock fields do not authorize a host to
bypass the adapter's transaction and fencing protocol. The store cannot prove
process quiescence and does not provide a separate read-only viewer.

`agent_kernel_resume(snapshot, run_id, fence)` preserves cumulative accounting.
A restored policy must validate both its original and current grants; full
access, auto-edit, interaction and child-depth limits cannot be widened by a
checkpoint. Plan's turn-scoped flag still clears at completion or interruption.
An active cleanup claim is valid only in the claimed or paused phase, and a
claimed phase cannot omit cleanup ownership. Contradictory snapshots fail.
A greater fence requires another committed checkpoint before dispatch. An
interrupted model request pauses; a claimed tool without a known result pauses
as unknown. A timeout or released OS lock does not prove an old tool has exited.
The host must first confirm actual old-executor/process-tree quiescence.

`agent_kernel_confirm_quiescence` is host-only: with current run/fence and
verified OS evidence, it clears cleanup ownership but leaves unknown work
paused. `agent_kernel_resolve_tool` accepts an externally verified result for
the current unresolved call, retains the unknown outcome as resolved audit
history, updates its model-context projection and commits before continuing.
It never executes the tool. Both require actual trusted host evidence; model
text, approval of a different call or a stale fence cannot provide it.

## Standalone example and verification

`examples/maintenance.sg` is a no-TUI host using actual sgllm streaming, SQLite,
file reads, one exact patch and a fixed verification command. The three fixture
tools are deliberately narrow and explicitly granted by that sample host; it is
not an arbitrary command sandbox. Its companion `tests/maintenance_fixture.py`
uses only loopback model responses and disposable projects, with two cases:

1. Commit after patch, exit, restart and run verification without patch replay.
2. Kill after the patch side effect but before committing its result; ordinary
   restart must refuse before a provider request and retain the one side effect.

Other tests cover pure compatibility APIs, complete kernel dispatch and Plan
guards, NUL control identities, malformed policy/snapshots, stale receipts,
host-only unknown resolution and repeated resource counters. Test evidence is
recorded only after actual execution with a frozen compiler/source set. Do not
replace crash/SQLite proof with a mock or claim all language ownership paths
sound from this package. Current language-supported ownership and async
cancellation boundaries still apply.
