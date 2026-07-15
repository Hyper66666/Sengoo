## Context

The runtime has a cooperative scheduler, reactor-backed timer/TCP subsets, a
blocking thread pool, cancellation/select primitives, and scalar transition
surfaces for channels and locks. The compiler now has structural `Send`/`Sync`
and source-level negative impls. The remaining gap is a generic, production-
shaped concurrency contract and release-host reactor evidence.

## Goals / Non-goals

- Goal: memory-safe generic multi-threading and a working async IO reactor on
  supported Windows, Linux, and macOS release hosts.
- Goal: structured task lifetime, cancellation, backpressure, and shutdown
  semantics independent of executor implementation.
- Non-goal: a high-level web framework or scheduler algorithm as part of the
  source-language contract.

## Decisions

### Decision 1: `Send`/`Sync` marker traits

`Send` means safe to move to another thread and `Sync` means `&T` is safe to
share. They are derived structurally when all fields/arguments satisfy the
marker. Runtime handles with single-thread affinity and `Rc<T>` have explicit
negative impls. Thread-spawn, channel-transfer, and shared-state APIs enforce
the appropriate bounds with stable diagnostics.

### Decision 2: Generic shared ownership and mutation

`Arc<T>` is atomic-refcounted shared ownership and is `Send`/`Sync` when its
payload permits. `Mutex<T>` and `RwLock<T>` own `T`; guards borrow the lock and
release on `Drop`. Generic payload representation reuses the type descriptor
and typed drop callback contract from `generic-collections` rather than adding
per-scalar storage families.

Locks must outlive guards. The v1 compiler enforces this conservatively:
acquisition borrows the lock for the lexical guard scope, moving that lock is
rejected while the borrow is active, and guard-bearing return types may only be
produced by the compiler-known stdlib acquisition functions. Arbitrary wrapper
functions cannot let a guard escape a borrowed lock. This is intentionally
stricter than a future NLL-style lifetime system. Moving a guard across threads
follows explicit marker rules. Poisoning is not required for v1, but
panic/error isolation and guard release are required.

### Decision 3: Executor contract is algorithm-independent

The cooperative single-thread scheduler remains the default. A selectable
multi-threaded executor accepts only `Send` futures. Its public contract covers:

- bounded submission and backpressure;
- progress and deterministic join results;
- cancellation and shutdown of accepted tasks;
- task failure isolation;
- no leaked tasks or worker threads.

A fixed worker pool satisfies the release contract. Work stealing may be added
later if reference benchmarks justify it and public semantics remain unchanged.

The v1 source/runtime boundary is explicit:

- `runtime_enable_executor(worker_count, capacity)` enables the executor; the
  capacity bounds all accepted, non-terminal tasks rather than only idle queue
  entries;
- `spawn_task(future)` transfers a directly constructed `Send` future to the
  executor when enabled and returns `0` when the bound rejects submission;
- a transferred future is pinned to one worker for all polls, so values created
  after transfer do not migrate between worker threads;
- `task_join(id)` waits for and returns a stable lifecycle status (`2`
  completed, `3` cancelled, `4` failed). Detached task outputs are deliberately
  discarded and their frames are dropped exactly once;
- terminal statuses remain queryable across explicit executor shutdown in a
  bounded process-local history of the newest 4096 task IDs; older IDs report
  `Unknown` rather than growing runtime memory without limit;
- async-main exit drains accepted tasks. Explicit shutdown selects drain or
  cancellation and rejects later submission.

Joinable `spawn(future)` retains its cooperative return-a-future behavior in
v1. Both `spawn` and `spawn_task` conservatively require directly constructed
`Send` futures so a future variable with unknown capture provenance cannot be
silently moved across an executor boundary in a later release.

### Decision 4: Cross-platform reactor

Platform backends use poll/epoll or equivalent on Linux, IOCP/handles or
equivalent on Windows, and kqueue/poll or equivalent on macOS. The shared
contract covers timers, sockets, and owned descriptor/handle readiness.

Registration, wakeup, timeout, cancellation, and close are generation-safe.
Reference-host tests prove progress without busy polling and verify stale
wakeups cannot target reused handles.

The v1 public file surface is deliberately owned and read-only:

```sengoo
let opened = async_file_open(path);
let ready = await opened.value.wait_readable(timeout_ms);
let count = opened.value.read_into(&mut buffer);
```

`AsyncFile` owns one runtime file entry and closes it on `Drop` or explicit
`close`. `wait_readable` returns `Future<FileReadinessOutcome>`, where success
has `value = true` and timeout has `error = STATUS_TIMEOUT`. Starting a wait
duplicates the underlying descriptor/handle into the reactor registration, so
the future never borrows a user-visible raw fd and remains memory-safe if the
source `AsyncFile` is closed while the wait is pending. Cancellation, future
Drop, and result consumption unregister that duplicate exactly once.

The `FileReadinessOutcome` runtime boundary uses one explicit caller-provided
output pointer on every host. It does not rely on the platform C ABI's small
aggregate return classification: Rust and generated LLVM otherwise disagree
about the SysV register representation of `{ bool, bool, i64 }`. The explicit
pointer keeps Windows MSVC, Linux SysV, and macOS SysV on one stable layout.

`read_into(&mut Buffer)` performs one bounded read into the initialized managed
buffer and reports the byte count or stable status category. It does not claim
general asynchronous disk throughput: regular files may be immediately ready,
while pipes and other supported pollable file kinds use the same reactor
backend and host support policy as the owned-handle tests. Raw descriptor
construction is not public in v1.

### Decision 5: Future and wakeup contract

`Future<T>::poll(&mut self, ctx: AsyncContext) -> Poll<T>` is the canonical
user-future shape. `AsyncContext` is an opaque by-value poll token because it
cannot escape the dynamic poll call; user code cannot construct, copy-store,
return, compare, or move it across threads.

`ctx.wake_after(delay_ms)` registers the earliest non-negative retry deadline,
and `ctx.wake()` is the zero-delay form. Every path that returns Pending must
register one of those wakeups. The compiler rejects an evident Pending path
without registration with the stable `async::user_future_missing_wakeup`
diagnostic; the runtime also converts a missing dynamic registration into a
bounded fallback retry rather than busy polling.

One cooperative owning task serializes calls to a user future. The generated
await control flow never calls poll again after Ready, and direct source-level
poll-after-Ready is unavailable because a valid `AsyncContext` cannot be
constructed or retained. User futures remain excluded from `select` and
cross-thread spawn in v1: select operands must be runtime future handles with
defined cancellation/drop dispatch, while inline user futures remain pinned to
their owning task. Stable negative tests document those boundaries.

### Decision 6: Generic channels

`channel<T>(capacity)` returns sender/receiver endpoints. Send moves `T` into
the channel and requires `T: Send` when crossing threads. The v1 receive
surface is `channel_recv_into<T>(&receiver, &mut initialized_output)`: success
drops the previous output value and moves the queued value into that storage
exactly once. This shape keeps all `T` supported without inventing an invalid
`Result<T, E>` placeholder or imposing a `Default` bound; a direct value-
returning convenience API may follow once the language has a matching
discriminated result representation. Close wakes waiters; queued values are
dropped exactly once; cancellation does not lose or double-drop values.
Capacity is bounded and backpressure is explicit. Compiler-known raw helpers
enforce the same `Send` boundary as the public wrapper and are not a safety
escape hatch.

### Decision 7: Structured concurrency

`task_scope` binds children to lexical scope. Normal exit joins children; early
return/error/panic cancels then joins them. Children cannot escape the scope,
and scope teardown has a bounded diagnostic timeout in tests.

The v1 source shape is deliberately small:

```sengoo
let scope = task_scope();
let accepted = scope_spawn(&scope, child());
```

`TaskScope` is an owned, non-`Send`, non-`Sync` compiler-known guard. Its lexical
owner must be introduced directly by `let scope = task_scope()`; local tuple,
array, constant, and other aggregate storage is rejected alongside aggregate
fields and return types.
`scope_spawn` accepts only a direct `Send` future, registers it before returning
`1`, and returns `0` after releasing the rejected future when the executor or
scope is closed. It does not expose a child task ID, so scoped children cannot
be joined or retained outside the guard.

MIR lowering distinguishes normal lexical fallthrough from early exits using
the existing drop-scope stack. Normal fallthrough emits join-all before the
guard's idempotent `Drop`; `return`, `?`, `break`, `continue`, contract abort,
and unwind cleanup reach `Drop` without that marker, so teardown cancels every
pending child and then joins all terminal states. The compiler rejects
`TaskScope` in return positions and aggregate fields rather than allowing a
guard to escape its lexical owner.

## Platform and evidence policy

- Feature/unit evidence does not imply host support.
- Windows, Linux, and supported macOS release jobs run the shared reactor and
  executor scenarios.
- A missing debugger/toolchain/runtime dependency is a visible skip during
  development but does not close the release-host gate.

## Migration

Existing scalar APIs remain compatibility wrappers until generic parity passes.
The cooperative executor remains default. Existing `select` remains non-
cancelling while `select_cancel` keeps its explicit semantics.

## Risks

- **Auto trait unsoundness:** recursive structural and explicit negative tests.
- **Drop races:** generation tokens, exact drop counters, sanitizer/leak stress.
- **Worker self-join:** executor workers help poll their own affinity queue while
  joining scoped children so a one-worker executor cannot deadlock on nested
  scope teardown.
- **Deadlock:** not prevented generally; bounded test timeouts and lock-order
  documentation are required.
- **Platform drift:** one shared scenario suite plus host-specific adapters.

## Archive gate

- generic Arc/locks/channel with Send/Sync and exact Drop;
- bounded multi-thread executor with join/cancel/shutdown stress;
- structured task scopes on normal and early exits;
- timer/socket/owned-handle reactor evidence on supported release hosts;
- docs/support matrix updated from host-tagged evidence.
