# Lifecycle and storage contract

The Agent kernel owns JSON state on one thread. It never holds credentials,
a Terminal, CliState or an OS tool process. A host drives `next_action` and feeds
validated completed provider results, permission decisions and known tool
results back into the kernel. Source shared with Sencode includes pure policy,
GoalState, request retry, task scheduling and tool scheduling; compatibility
names remain available through thin CLI imports. The dynamic registry is separate
from the retained builtin tool-name profile.

## Durable action protocol v1

`AgentKernel` contains an owned checkpoint and a dirty flag. While dirty, its
only action is `persist`. That action has:

```text
schema: 1
type: "persist"
id: "<run_id>:commit:<revision>"
run_id: stable logical run identity
fence: positive host-acquired execution fencing token
revision: proposed revision, starting at 1
checkpoint: complete prospective state with the same run_id/fence/revision
```

The checkpoint contains schema, run_id, fence, revision, phase, sequence,
requests, tokens, maximum_requests, token_budget, messages, tools, target,
policy, initial_policy, calls, call_index, ledger, reason, pending_request,
last_request, last_output, attempt, retry_due, retry_waited, cleanup_required,
unresolved_call and optional host_facts.

The host's atomic commit returns
`AgentCommitReceipt { run_id: String, fence: i64, revision: i64 }` only after
durable success. A new write requires durable revision + 1. A duplicate of the
current revision is accepted only if the entire checkpoint is byte-identical.
Stale, skipped or conflicting revisions fail with status12. The adapter validates
run, fence, schema and revision both outside and inside the checkpoint. Kernel
acknowledgment must match its captured run/fence/revision exactly.

The in-memory ledger tests this replaceable interface; it is not persistence
proof. The SQLite adapter owns OS locks, its lease/fence, transaction boundaries
and receipt production. Sencode maps the same contract onto its authoritative
shared store without a second writer or incompatible migration.

## Phases and execution

Phases are 1 ready-for-model, 2 model-request-admitted, 3 choosing-next-tool,
4 awaiting-permission, 5 tool-claim-admitted, 6 retry-wait, 9 complete, 10 paused.
Every transition affecting dispatch or outcome first creates a new checkpoint.
After a matching commit, `model_request` or `tool_request` may be dispatched.
Stable action IDs make polling idempotent; a host must never dispatch one action
twice. Requests reserve cumulative request accounting before dispatch; token
usage is added from accepted provider results and never reset on recovery.
Zero request limit means no request-count cap. Explicit exact token budgets
pause when measured usage is unavailable.

Provider results must be complete canonical assistant turns with unique call
IDs, full tool names/arguments and valid accounting. Repeated acceptance of the
same request ID does not duplicate history. Reuse of a completed tool ID is
rejected; no tool side effect is repeated to implement a request retry.
An explicit canonical `requires_continuation=true` with no tool calls continues
the same logical task and cumulative budget instead of completing early. The
kernel appends no invented user instruction to implement this protocol signal.
Malformed usage types are rejected before changing the checkpoint; reported
exact usage requires both integer token counts.

Tools are registered with schema, read_only, process and other declared metadata.
Unknown schemas or invalid fields fail before dispatch. Schema success does not
grant permission. Plan permits reads and blocks process/write tools; captured
manual mode emits a separate approval action, auto-edit grants edits only, and
full access is an explicit host policy. A tool result cannot update policy.
Plan clears on complete, pause and cancellation; initial_policy preserves the
historical request policy. Terminal UI and approval freshness remain host concerns.

The kernel's retry transition is requested by the host and uses the same bounded
five-retry policy as Sencode. Waiting produces a pollable action, not a sleep.
Cancellation and unknown effects pause. A known tool result is accepted only
after the host confirms quiescence; unconfirmed outcome or cleanup produces a
paused checkpoint and cannot release write authority or replay a tool.

## Resume and corruption

Resume validates identity, schema, nonnegative accounting, phase and fence.
Both original and current policy objects must have exact boolean/integer field
shapes. The current grant cannot increase full access, auto-edit, interaction
or child-depth authority beyond the original capture. The turn-scoped Plan
overlay may still end after completion or interruption. Cleanup ownership is
consistent with claimed/paused phases, a claim cannot omit cleanup ownership,
and pending calls have unique control IDs. Contradictory snapshots are rejected
before they become live state or produce an action.
A new fence must be at least the saved fence and requires a new checkpoint
before any new dispatch. The host must prove the old executor and owned process
tree quiescent before acquiring a new fence. OS lock expiry or heartbeat timeout
alone is insufficient. In-flight model requests pause as interrupted; claimed
tools without a known result pause as outcome unknown. No automatic re-execution
is inferred from the absence of a terminal result. A host may reconstruct a new
authoritative checkpoint only after explicitly verifying an uncertain outcome.

The host-only confirmation APIs are separate from tool inputs. With a matching
current run/fence, `agent_kernel_confirm_quiescence` records actual OS/process
cleanup evidence and clears cleanup_required while leaving the task paused.
`agent_kernel_resolve_tool` requires verified/quiescent host input, a matching
current unresolved call ID and evidence text. It records the old unknown ledger
entry as resolved, retains its audit content, and appends the confirmed result
without executing the tool. Only the model-context projection of the unknown
result is replaced. The result transition must be committed before the next
action. Neither a model assertion nor releasing/reacquiring an OS lock provides
this proof. The SQLite adapter refuses ordinary close or takeover while an
uncleared cleanup requirement remains; explicit recovery admission still keeps
the unknown outcome paused until host verification.

## Verification

Retained scheduler, policy/retry and tool-schedule cases run through the new
package. Kernel tests drive commit gates, approval, exact budgets, cancellation,
duplicate/unknown tool IDs, stale receipts, callback-free dispatch and restored
snapshots. Resource probes cover repeated failed construction and checkpoints.
A real SQLite store test proves atomicity, two owners, fencing and crash cases.
An external no-TUI application then uses actual providers and registered read,
patch and test tools against a disposable project, resumes without replay, and
uses locked dependencies from outside the repositories. Loopback and live API
evidence remain separate; mock persistence cannot satisfy the real-store gate.
