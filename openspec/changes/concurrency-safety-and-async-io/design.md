## Context

The runtime has a cooperative scheduler, reactor-backed timers/TCP, a blocking
thread pool (`spawn_blocking`), and `select`/`spawn_task`/`cancel_task`
(`docs/runtime-async-semantics.md`). The gaps are a safety model for real
threads and a proven cross-platform fd-readiness reactor.

## Goals / Non-goals

- Goal: memory-safe multi-threading and a working async IO reactor on Linux and
  Windows.
- Non-goal: a particular high-level async framework; macOS reactor parity.

## Decisions

### Decision 1 — `Send`/`Sync` marker traits

`Send` (safe to move to another thread) and `Sync` (`&T` safe to share) are
auto-implemented structurally: a type is `Send`/`Sync` if all fields are. Types
holding non-thread-safe handles (e.g. `Rc<T>`, raw single-thread handles) are
`!Send`/`!Sync`. Thread spawn and shared-state APIs require the appropriate
bound; violations are a stable diagnostic.

### Decision 2 — Shared ownership and mutation

`Arc<T>` is atomic-refcounted shared ownership (`Send`/`Sync` when `T: Send + Sync`).
`Mutex<T>`/`RwLock<T>` provide interior mutability with `lock()`/`read()`/`write()`
guards that `Drop`-release the lock (RAII), tying into
`automatic-memory-management`.

### Decision 3 — Executor

Add a multi-threaded work-stealing executor selectable at runtime startup,
keeping the cooperative single-thread executor as the default for simple
programs. `spawn` on the multi-threaded executor requires a `Send` future.

### Decision 4 — Reactor

A platform reactor abstraction with epoll/poll (Linux) and IOCP/handle (Windows)
backends drives timer, socket, and owned-fd readiness. This closes the
`async-default-followups` / `async-reactor-futures` "all-host owned-fd readiness"
deferral for Linux + Windows, proven by reference-host tests.

### Decision 5 — `Future` trait and channels

```sg
trait Future {
    type Output;
    def poll(&mut self, cx: &mut Context) -> Poll<Output>;
}
```

Generalize the existing user-future subset to this trait. `channel<T>()` returns
a `(Sender<T>, Receiver<T>)` mpsc pair; send/recv integrate with the reactor for
async waiting.

### Decision 6 — Structured concurrency

A `task_scope` helper spawns child tasks bound to a scope; on scope exit all
children are joined (or cancelled on early exit), guaranteeing no leaked tasks.

## Risks / Trade-offs

- **Auto Send/Sync correctness.** Mitigation: explicit negative tests for
  `!Send` types crossing thread boundaries.
- **Reactor portability.** Mitigation: per-platform backend with a shared test
  suite; macOS deferred to a later channel, documented honestly.
- **Deadlocks.** Out of scope to prevent; document lock-ordering guidance.

## Migration

Cooperative scheduler remains the default; multi-threaded executor and `Arc`/
locks are opt-in. Existing async examples keep working; `select`/user-future
relaxations are additive with retained negative tests.
