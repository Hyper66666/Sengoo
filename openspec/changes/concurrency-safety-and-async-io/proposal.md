## Why

Sengoo markets "no GIL, true parallelism," but the implemented runtime is a
single-thread cooperative scheduler with an optional blocking thread pool, and:

- there is **no data-race safety model** (no `Send`/`Sync` equivalent), so true
  multi-threaded sharing is unsafe by construction;
- the async IO reactor is documented as platform-specific and unproven on POSIX
  ("owned-fd all-host readiness" is Deferred in
  `examples/realworld/SUPPORT_MATRIX.md`);
- `select` is limited to 2..8 homogeneous operands and user-defined futures are
  a restricted subset.

A mainstream concurrency story needs a safety model and a working cross-platform
async IO layer. This depends on `automatic-memory-management` (thread-safe
sharing needs `Drop` + ownership) and `generics-and-trait-system` (`Send`/`Sync`
marker traits, `Future` trait).

## Proposal

- **Data-race safety model**: `Send` and `Sync` marker traits (auto-derived,
  with documented negative cases), enforced at thread-spawn and shared-state
  boundaries. `Arc<T>` (atomic refcount) for thread-safe shared ownership, and
  `Mutex<T>`/`RwLock<T>` for shared mutation.
- **Multi-threaded executor**: a selectable executor alongside the cooperative
  scheduler, with `spawn` requiring `Send` futures. The release contract fixes
  correctness, cancellation, backpressure, and deterministic join behavior;
  work stealing is an optional optimization justified by benchmarks.
- **Cross-platform async IO reactor**: timer, socket, and owned-handle readiness
  on Linux, Windows, and the supported macOS release channel, proven by
  reference-host tests without busy polling.
- **General `Future` trait** with a documented `poll` contract, removing the
  user-future subset restrictions where sound.
- **Channels**: `channel<T>()` (mpsc) for message passing between tasks/threads.
- **Structured concurrency helpers**: scoped task groups that join/cancel
  children on scope exit.

## What changes

- ADDED: `Send`/`Sync` model + enforcement; `Arc<T>`, `Mutex<T>`, `RwLock<T>`.
- ADDED: multi-threaded executor with algorithm-independent public semantics.
- ADDED: cross-platform IO reactor for timers/sockets/owned handles on supported
  release hosts.
- ADDED: general `Future` trait and channels; structured task groups.
- MODIFIED: relax `select` and user-future restrictions where sound, keeping
  negative tests for unsound escapes.

## Non-goals

- A specific async ecosystem (HTTP frameworks, etc.) — only the runtime + safety
  primitives.
- Scheduler-specific performance promises before stable reference benchmarks;
  work stealing remains optional.
