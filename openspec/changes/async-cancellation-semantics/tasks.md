## 1. Pinning

- [x] 1.1 Run `openspec validate async-cancellation-semantics --strict`.
- [x] 1.2 Pin `STATUS_CANCELED` value in `tools/stdlib/status.sg` taxonomy
  and the final cancellable-wait helper name; record both in `design.md`
  before code edits.

## 2. Task cancellation propagation

- [ ] 2.1 Runtime: canceled mark observed at next await point; frame does
  not resume user code past that await; status reaches `3`.
- [ ] 2.2 Awaiting a canceled spawned future resolves to the
  `STATUS_CANCELED` error path; completed tasks never demote.
- [ ] 2.3 Dropped child futures of a canceled frame unregister reactor
  interest (assert via reactor registration counters).
- [ ] 2.4 Compiler + native tests: post-await code never runs after cancel;
  pending/completed/canceled transitions covered.

## 3. `select_cancel`

- [ ] 3.1 Stdlib + lowering for `select_cancel` 2..8 homogeneous operands
  with rotating poll order (mirror `select` arity/type diagnostics).
- [ ] 3.2 Runtime: winner returned; losers canceled-then-dropped before
  return; no dangling reactor interest (drop-order assertions).
- [ ] 3.3 Spawned-task losers transition underlying tasks to canceled.
- [ ] 3.4 Native tests with 2, 3, and 8 operands plus a mixed
  spawned/plain-future loser case; existing `select` tests stay green
  unchanged.

## 4. Cancellable process wait

- [ ] 4.1 Runtime cancellable wait: exit → code; timeout →
  `STATUS_TIMEOUT`; kill during wait → prompt `STATUS_CANCELED`.
- [ ] 4.2 Stdlib wrapper on `ProcessHandle` with generation checks and
  existing `STATUS_INVALID_HANDLE` mapping.
- [ ] 4.3 Tests on Windows and POSIX: kill-during-wait resolves promptly
  (bounded by a small grace assertion, not the full timeout).

## 5. Docs and matrix

- [ ] 5.1 Update `docs/runtime-async-semantics.md` with the cancellation
  contract, `select_cancel`, and the process wait semantics.
- [ ] 5.2 Move the three Deferred SUPPORT_MATRIX rows (task cancellation
  boundaries, select loser cancellation, process cancellation) to supported
  subsets with proof links.

## 6. Verification

- [ ] 6.1 `cargo fmt --check`
- [ ] 6.2 `cargo test -p sengoo-compiler --lib`
- [ ] 6.3 `cargo test -p sengoo-runtime --lib --features native-bridge -- --test-threads=1`
- [ ] 6.4 `cargo test -p sgc async_native_runtime -- --test-threads=1`
- [ ] 6.5 `openspec validate async-cancellation-semantics --strict`

## Archive Gate

- [ ] `openspec validate async-cancellation-semantics --strict` passes.
- [ ] Canceled tasks provably run no post-await user code; status machine
  covered end to end.
- [ ] `select_cancel` determinism and no-dangling-interest assertions pass
  for 2..8 operands; existing `select` semantics unchanged.
- [ ] Cancellable process wait proven prompt on Windows and POSIX.
- [ ] Matrix rows moved with proof; umbrella records Pillar B completion.
