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

### Decision 4: Cross-platform reactor

Platform backends use poll/epoll or equivalent on Linux, IOCP/handles or
equivalent on Windows, and kqueue/poll or equivalent on macOS. The shared
contract covers timers, sockets, and owned descriptor/handle readiness.

Registration, wakeup, timeout, cancellation, and close are generation-safe.
Reference-host tests prove progress without busy polling and verify stale
wakeups cannot target reused handles.

### Decision 5: Future and wakeup contract

`Future<T>::poll(&mut self, cx: &mut AsyncContext) -> Poll<T>` is the canonical
user-future shape. Pending futures must arrange a wakeup or be documented as
manual-poll values. Concurrent/reentrant polling of one future is rejected or
serialized by its owning task. Poll-after-Ready is invalid and covered by
negative tests.

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
- **Deadlock:** not prevented generally; bounded test timeouts and lock-order
  documentation are required.
- **Platform drift:** one shared scenario suite plus host-specific adapters.

## Archive gate

- generic Arc/locks/channel with Send/Sync and exact Drop;
- bounded multi-thread executor with join/cancel/shutdown stress;
- structured task scopes on normal and early exits;
- timer/socket/owned-handle reactor evidence on supported release hosts;
- docs/support matrix updated from host-tagged evidence.
