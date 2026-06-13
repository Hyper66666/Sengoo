## Context

The cooperative scheduler (`CoroutineScheduler` + reactor) already supports
pending-task cancellation (`cancel_task` flips a pending task to canceled),
consuming `timeout_cancel`, and reactor-interest cleanup on drop (proven by
the async HTTP pending-drop path). The umbrella froze decision D3:
cancellation stays cooperative at await points; `select_cancel` is a new
consuming variant; the process wait gains a cancellation-capable form.

## Decisions

### D-B1 Task cancellation propagation

- `cancel_task(task_id)` marks the task canceled. If the task is mid-poll,
  the mark is observed at its **next await point**: the frame does not
  resume user code past that await; the task transitions to status `3`.
- Status machine stays `0=unknown, 1=pending, 2=completed, 3=canceled`;
  completed tasks are never demoted; canceling an unknown id returns
  `false` as today.
- Plain `await Future<T>` remains type-stable and returns `T`; this change
  does not retrofit ordinary awaits into `Result<T, i64>`. Cancellation
  status is observed through `task_status(task_id)` for scheduled tasks and
  through dedicated status-returning futures / stdlib APIs such as
  `timeout_cancel` and `ProcessHandle.wait_cancellable`, using
  `STATUS_CANCELED() == 19` with no renumbering of existing categories.
- Child futures owned by a canceled frame are dropped through normal future
  cleanup, which unregisters any reactor interest (reuse of the HTTP-drop
  invariant).

### D-B2 `select_cancel`

- Signature mirrors `select`: homogeneous operands, arity 2..8, rotating
  poll order (same fairness rule as `select`).
- First `Ready` branch in the current poll order wins; its value is
  returned; every loser is immediately marked canceled and dropped before
  `select_cancel` returns.
- Determinism contract: after `select_cancel` returns, no loser can
  complete, run user code, or hold reactor interest registrations.
- Losers that are spawned-task handles transition the underlying task to
  canceled per D-B1; plain futures are dropped.
- `select` keeps its pinned non-canceling semantics verbatim.

### D-B3 Cancellable process wait

- New stdlib helper on the generation-checked handle:
  `ProcessHandle.wait_cancellable(timeout_ms) -> Result<i64, i64>`.
- Resolution cases: process exit -> exit code; timeout -> `STATUS_TIMEOUT`;
  process killed (by this handle's `kill` from another task or by the
  runtime) -> resolves within 250 ms on the reference CI host with
  `STATUS_CANCELED() == 19` instead of blocking out the full timeout.
- Stale/closed handles keep the existing `STATUS_INVALID_HANDLE` mapping;
  generation checks unchanged.
- Windows and POSIX both covered; host-specific wakeup mechanics live in
  the runtime, not the stdlib surface.

### D-B4 Verification approach

- Compiler tests: cancellation propagation lowering (canceled task does not
  resume past await), `select_cancel` arity/type checks mirroring `select`
  negative tests.
- Native runtime tests (`-p sengoo-runtime --features native-bridge`, then
  `-p sgc` e2e): post-await code never runs after cancel; status reaches 3;
  `select_cancel` with 2, 3, and 8 operands; loser reactor-interest
  assertions; process kill-during-wait resolves promptly on both hosts.
- Matrix rows move only with these proofs linked.

## Risks / Trade-offs

- Cancel-at-await means long synchronous stretches delay cancellation; this
  is the documented cooperative model (umbrella non-goal: no preemption).
- Loser cancellation must not race reactor wakeups: drop order is
  cancel-mark → unregister interest → drop frame, asserted by tests, to
  avoid waking a freed frame (the known dangling-interest risk from the
  umbrella).
- Prompt kill-wakeup on Windows requires waiting on the process handle and
  a cancel event together; encapsulated in the runtime so the stdlib
  surface stays host-neutral.

## Migration Plan

Purely additive. Existing programs using `select`, `timeout`,
`timeout_cancel`, and `wait(timeout_ms)` are unaffected.

## Open Questions

- None before implementation. Future expansion to cancellation tokens or
  structured scopes requires a separate OpenSpec.
