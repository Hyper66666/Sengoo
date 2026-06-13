## Why

Three SUPPORT_MATRIX rows — task cancellation boundaries, select loser
cancellation, and process cancellation — are `Deferred` with no active
owning change since `async-default-followups` archived. The runtime already
has the mechanical pieces (`spawn_task`/`cancel_task`/`task_status` with
states `0..3`, consuming `timeout_cancel`, reactor interest cleanup proven
by the async HTTP drop path, and `ProcessHandle.kill`), but there is no
specified contract for *stopping work you started*: cancellation propagation
is undefined beyond pending tasks, `select` never cancels losers, and a
process wait cannot be interrupted. Production async services cannot be
written confidently without these semantics. This is Pillar B of the
`mainstream-adoption-gap-closure` umbrella.

## What Changes

- Specify and test the cooperative task-cancellation contract on the
  existing API: a canceled spawned task stops at its next await point,
  reaches the canceled terminal state (`task_status == 3`), and runs no
  subsequent user code; already-completed tasks stay completed.
- Add consuming `select_cancel(f1..fN)` for 2..8 homogeneous operands: the
  first ready branch wins and is returned; losers are canceled and dropped
  deterministically with no dangling reactor interest registrations. The
  existing non-canceling `select` is unchanged.
- Add a cancellation-capable process wait on the generation-checked
  `ProcessHandle` that resolves promptly when the process is killed or the
  wait is canceled, with a stable status; existing `wait(timeout_ms)`/
  `kill`/`exit_code`/`close` behavior is unchanged.
- Move the three Deferred matrix rows to supported subsets with proof rows.

## Capabilities

### New Capabilities

- `async-cancellation`: the cooperative cancellation contract for spawned
  tasks, the consuming loser-canceling select variant, and the
  cancellation-capable process wait.

### Modified Capabilities

- None. The pinned non-canceling `select` policy, `timeout`/`timeout_cancel`
  semantics, `Poll<T>`/`Future<T>` contract, and `ProcessHandle` lifecycle
  in `async-reactor-futures`, `async-default-followups`, and
  `stdlib-mainstream-usability` canonicals remain as specified; all new
  behavior is additive surface.

## Impact

- `runtime/src/async_runtime/` (cancellation propagation at await points,
  loser cancel/drop, reactor interest cleanup), `runtime/src/` process
  runtime (cancellable wait), `tools/stdlib/` async/process wrappers,
  compiler async lowering tests, `tools/sgc` native async tests,
  `docs/runtime-async-semantics.md`, `examples/realworld/SUPPORT_MATRIX.md`.
- Parent umbrella: `mainstream-adoption-gap-closure` (Pillar B); design
  decision D3 is frozen there.
- Pillar C (`http-production-serving`) consumes the loser-cancellation
  primitives for connection teardown and is sequenced after this change.

## Non-Goals

- No preemptive cancellation: work between await points always runs to the
  next await.
- No cancellation tokens, contexts, or structured-concurrency scopes; task
  ids and handles remain the only cancellation capabilities.
- No change to `select` (non-canceling), `timeout`, or `timeout_cancel`.
- No async drop/finalizer language feature; cleanup remains normal future
  drop.
