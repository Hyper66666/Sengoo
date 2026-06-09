## Scope

This child change tracks async features that remain too partial for
mainstream-default claims after the current async subset.

## Current Supported Subset

- Cooperative scheduler with timer/TCP reactor wakeups.
- Platform-specific owned-fd readiness for Unix poll-backed fds and Windows
  disk/pipe handles through `native-bridge`.
- `select(2..8)` over homogeneous futures.
- Non-canceling `timeout` and canceling `timeout_cancel`.
- Opt-in thread pool, `spawn_blocking_i64`, bounded channels, and mutex helpers.
- Realworld package smoke: `examples/realworld/async-channel-smoke`.

## Follow-Up Ownership

| Gap | Required decision before support claim |
| --- | --- |
| User-defined `Future::poll` lowering | Frame ownership, poll receiver rules, diagnostics, native tests |
| All-host owned-fd readiness | Host support matrix, stable unsupported statuses, platform skip evidence |
| Cancellation boundaries | Which handles/tasks can be canceled, cleanup guarantees, loser policy |
| Public cleanup wrappers | Correct void lowering for package-shaped async code |
| `sglsp` parity | Same diagnostics/ranges as compiler for accepted and rejected async shapes |

## Pinned V1 Decisions

This change treats the existing `tools/stdlib/async_futures.sg` surface as the
canonical public contract:

```sg
struct Poll<T> {
    is_ready: bool,
    value: T,
}

struct AsyncContext {
    handle: i64,
}

trait Future<T> {
    def poll(&mut self, ctx: AsyncContext) -> Poll<T>;
}
```

Implementation must not invent alternate `Pending`/`Ready` enum syntax in this
lane. `Poll { is_ready: false, value: ... }` is Pending; `is_ready: true` is
Ready and terminal.

V1 user futures are same-thread cooperative futures only:

- `await value` may consume a compiler-generated future handle or a user value
  implementing `Future<T>`.
- The poll receiver is exclusive for the dynamic poll call. The compiler/runtime
  must prevent concurrent or reentrant poll of the same future.
- A Ready future must not be polled again.
- `AsyncContext` is opaque and poll-scoped. User source may receive it only as a
  `poll` parameter and may not construct, store, return, compare, capture for
  `spawn_blocking`, or place it in a struct/global.
- A Pending poll must preserve the same future value and state for a later poll;
  lowering must not clone or move the user future into a fresh slot on resume.

Cancellation remains bounded in this change:

- `timeout_cancel` consumes the inner future, drops/cancels it on timeout, and
  returns `STATUS_TIMEOUT`.
- `select(2..8)` continues to drop losing operands normally. This change does
  not add select-loser cancellation unless implementation also adds a dedicated
  user-facing API, cleanup tests, and support-matrix wording.
- Task cancellation may be documented only for task handles whose lifecycle is
  visible through runtime APIs and whose cleanup/drop hooks are tested.

All-host owned-fd readiness may be claimed only after each host policy is named.
Otherwise the existing platform-specific subset remains the supported claim and
all-host readiness remains Deferred.

Payload-carrying enum values crossing `await` are owned by
`language-default-polish`. If user-future or cancellation implementation touches
async frame layout for those values, this change must coordinate with that
language child and keep payload-enum support Deferred until both specs' tests
and support-matrix claims agree.

## Evidence Strategy

Each promoted feature needs:

- Compiler tests for accepted and rejected source forms.
- Runtime/native tests for scheduler and cleanup behavior.
- Realworld package smoke when the feature is intended for user workflows.
- `docs/runtime-async-semantics.md` and `SUPPORT_MATRIX.md` rows updated in the
  same change.

## Compatibility

Current public APIs remain source-compatible. Unsupported shapes must fail with
stable diagnostics or `STATUS_UNSUPPORTED` rather than unresolved native symbols
or invalid LLVM.
