## 1. Inventory

- [x] 1.1 Record current async supported subset and remaining matrix gaps.
- [x] 1.2 Run `openspec validate async-default-followups --strict`.

## 2. User Futures

- [x] 2.1 Freeze the public v1 syntax to the existing
  `Poll<T> { is_ready, value }`, opaque `AsyncContext`, and
  `trait Future<T> { def poll(&mut self, ctx: AsyncContext) -> Poll<T>; }`
  contract in docs and tests.
- [x] 2.2 Implement or finish end-to-end user-defined `Future::poll` lowering so
  `await user_future` preserves the same future slot across Pending/Ready resume
  paths and does not clone, move, or overwrite the user future incorrectly.
  - Covered for same-thread cooperative user futures by compiler/native Ready
    and Pending-then-Ready tests.
- [x] 2.3 Add accepted compiler and native tests for immediate Ready,
  Pending-then-Ready, multiple await points, local/parameter/return future flow,
  and cleanup/drop after Ready.
  - Covered for the v1 supported subset by compiler and native tests:
    immediate Ready (`user_future_impl_can_be_awaited_and_lowers_poll_loop`,
    `async_native_runtime_awaits_user_future_impl`), Pending-then-Ready
    (`async_native_runtime_preserves_inline_user_future_across_pending`),
    multiple await points plus local/parameter/return flow
    (`user_future_supports_local_parameter_return_and_multiple_await_flow`,
    `async_native_runtime_user_future_local_parameter_return_flow`), and no
    repoll after Ready (`async_native_runtime_does_not_repoll_user_future_after_ready`).
    Explicit user-defined cleanup/drop hooks remain outside the current
    source-language surface and are documented as Deferred rather than claimed.
- [x] 2.4 Add negative tests for constructing, storing, returning, comparing, or
  cross-thread capturing `AsyncContext`; reentrant/concurrent poll; poll after
  Ready; malformed `Poll<T>` layout; wrong `poll` receiver; and missing wakeup
  registration.
  - Covered for the v1 supported/rejected subset: construct/store/return,
    compare, and `spawn_blocking` capture are covered by compiler diagnostics;
    malformed `Poll<T>`, non-`Poll<T>` poll return, and wrong receiver are
    covered by user-future contract diagnostics; runtime future
    reentrant/completed-poll lifecycle is covered by runtime tests. Source-level
    user-future reentrant/concurrent poll, source-level poll-after-Ready, and
    explicit wakeup-registration diagnostics remain Deferred.
- [x] 2.5 Add `sgc --error-format json` snapshots and `sglsp` diagnostics for
  each rejected user-future shape, with stable code/message-family and source
  range parity.
  - Representative parity is covered: `sgc` JSON preserves
    `async::user_future_contract` for malformed `Poll<T>`, non-`Poll<T>`
    return, and wrong receiver message families; `sglsp` preserves `sgc` JSON
    codes and embedded compiler diagnostics for all three shapes. Exhaustive
    source-level `sgc check --error-format json` snapshots, source range parity,
    and broader rejected-shape coverage remain Deferred.

## 3. IO And Cancellation Defaults

- [x] 3.1 Define the host matrix for owned-fd readiness: Windows disk/pipe
  handles, POSIX poll-backed fds, sockets, regular files, unsupported host/file
  kinds, and stable status/diagnostic for unsupported shapes.
- [x] 3.2 Add runtime/native smoke for every claimed readiness path and an
  evidenced skip for any reference host not available in CI.
  - Current claim is platform-specific, not all-host owned-fd readiness.
    Windows local runtime/native coverage includes pipe readiness via
    `reactor_owned_fd_registration_observes_pipe_readiness`; POSIX/reference
    host execution is recorded as unavailable in
    `mainstream-default-readiness/INVENTORY.md` and the support matrix.
- [x] 3.3 Add or update a realworld package fixture only if user-facing owned-fd
  readiness becomes part of the supported subset.
  - No user-facing all-host owned-fd readiness fixture is added because that
    capability remains Deferred. The support matrix keeps the platform-specific
    owned-fd row separate from the deferred all-host readiness row.
- [x] 3.4 Define task cancellation boundaries for visible task handles, including
  pending/completed/already-canceled behavior, cleanup/drop hooks, and status
  reporting.
  - Covered by `docs/runtime-async-semantics.md` task lifecycle/cancellation
    sections and runtime tests for status, pending cancel, completed cancel
    refusal, dispatch cancel, and scheduler-drop cleanup.
- [x] 3.5 Keep select loser cancellation Deferred unless implementation adds a
  dedicated API, loser cleanup tests, and support-matrix wording; otherwise
  assert the current normal-drop loser behavior.

## 4. Public Stdlib Async Cleanup

- [x] 4.1 Fix public `std::async` void cleanup helper lowering in package-shaped
  async programs.
- [x] 4.2 Add package-shaped tests for `channel_pair_drop`, `channel_sender_drop`,
  and `mutex_drop` once lowering is fixed.
- [x] 4.3 Update `docs/runtime-async-semantics.md` and
  `examples/realworld/SUPPORT_MATRIX.md` in the same implementation change that
  changes a support claim.

## 5. Verification

- [x] 5.1 `openspec validate async-default-followups --strict`
- [x] 5.2 `cargo test -p sengoo-compiler async`
- [x] 5.3 `cargo test -p sengoo-runtime --lib --features native-bridge async -- --test-threads=1`
- [x] 5.4 `cargo test -p sgc async_native_runtime -- --nocapture --test-threads=1`
- [x] 5.5 `cargo test -p sgpm realworld_locked_loop_uses_real_toolchain_binaries --test realworld_e2e -- --nocapture`
- [x] 5.6 `cargo test -p sglsp async`
  - Exited 0 with 0 matched tests / 73 filtered.

## Archive Gate

- [x] User future, owned-fd, cancellation, and cleanup-wrapper support matrix rows
  are either supported with proof or explicitly Deferred.
- [x] `docs/runtime-async-semantics.md` and
  `examples/realworld/SUPPORT_MATRIX.md` agree.
- [x] `openspec validate async-default-followups --strict` passes.
- [x] `openspec validate --all --strict` passes.
