## 1. Pinning

- [x] 1.1 Run `openspec validate async-cancellation-semantics --strict`.
- [x] 1.2 Pin `STATUS_CANCELED` value in `tools/stdlib/status.sg` taxonomy
  and the final cancellable-wait helper name; record both in `design.md`
  before code edits.

## 2. Task cancellation propagation

- [x] 2.1 Runtime: canceled mark observed at next await point; frame does
  not resume user code past that await; status reaches `3`.
- [x] 2.2 Cancellation status remains observable through `task_status` and
  status-returning futures/APIs without changing ordinary `await Future<T>`
  result types; completed tasks never demote.
- [x] 2.3 Dropped child futures of a canceled frame unregister reactor
  interest (assert via reactor registration counters).
- [x] 2.4 Compiler + native tests: post-await code never runs after cancel;
  pending/completed/canceled transitions covered.

## 3. `select_cancel`

- [x] 3.1 Stdlib + lowering for `select_cancel` 2..8 homogeneous operands
  with rotating poll order (mirror `select` arity/type diagnostics).
- [x] 3.2 Runtime: winner returned; losers canceled-then-dropped before
  return; no dangling reactor interest (drop-order assertions).
- [x] 3.3 Spawned-task losers transition underlying tasks to canceled.
- [x] 3.4 Native tests with 2, 3, and 8 operands plus a mixed
  spawned/plain-future loser case; existing `select` tests stay green
  unchanged.

## 4. Cancellable process wait

- [x] 4.1 Runtime cancellable wait: exit -> code; timeout ->
  `STATUS_TIMEOUT`; kill during wait -> prompt `STATUS_CANCELED`.
- [x] 4.2 Stdlib wrapper on `ProcessHandle` with generation checks and
  existing `STATUS_INVALID_HANDLE` mapping.
- [x] 4.3 Tests on Windows and POSIX: kill-during-wait resolves promptly
  (bounded by a small grace assertion, not the full timeout). Windows local
  proof is covered by `stdlib_process_wait_cancellable_returns_promptly_after_kill`;
  POSIX proof is covered by PR #17 Ubuntu `core-conformance`
  `cargo test --workspace --locked -- --test-threads=1`.

## 5. Docs and matrix

- [x] 5.1 Update `docs/runtime-async-semantics.md` with the cancellation
  contract, `select_cancel`, and the process wait semantics.
- [x] 5.2 Update the SUPPORT_MATRIX rows with proof links: task
  cancellation boundaries, select loser cancellation, and process
  cancellation are supported subsets.

## 6. Verification

- [x] 6.1 `cargo fmt --check`
- [x] 6.2 `cargo test -p sengoo-compiler --lib`
- [x] 6.3 `cargo test -p sengoo-runtime --lib --features native-bridge -- --test-threads=1`
- [x] 6.4 `cargo test -p sgc async_native_runtime -- --test-threads=1`
- [x] 6.5 `openspec validate async-cancellation-semantics --strict`

## Archive Gate

- [x] `openspec validate async-cancellation-semantics --strict` passes.
- [x] Canceled tasks provably run no post-await user code; status machine
  covered end to end.
- [x] `select_cancel` determinism and no-dangling-interest assertions pass
  for 2..8 operands; existing `select` semantics unchanged.
- [x] Cancellable process wait proven prompt on Windows and POSIX.
- [x] Matrix rows updated with proof links and supported-subset process
  cancellation status.
- [x] Umbrella records Pillar B completion after POSIX process prompt proof.
