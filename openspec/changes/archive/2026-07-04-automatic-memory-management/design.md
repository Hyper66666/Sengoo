## Context

The compiler already lowers through HIR → MIR and has a borrow checker plus
locals with `LocalKind` (Temp / Param / User). Drop insertion is a MIR-level
transformation that runs after borrow checking and before codegen. Runtime
resources are currently C-runtime handles (`Buffer`, collection handles,
`JsonDoc`, process/net handles) with explicit free functions in
`tools/stdlib/runtime*.c` and Sengoo wrappers in `tools/stdlib/*.sg`.

## Goals / Non-goals

- Goal: deterministic, automatic release at end of owner scope for all owning
  types, with no runtime GC.
- Goal: keep generated IR and the C/Rust-class runtime performance claim intact.
- Non-goal: shared-ownership defaults, cycle collection, or finalizer ordering
  guarantees beyond reverse-declaration order within a scope.

## Decisions

### Decision 1 — `Drop` trait shape

```sg
trait Drop {
    def drop(&mut self);
}
```

`drop` is never called directly by users; the compiler calls it. Calling
`x.drop()` explicitly is allowed only through the existing compatibility methods
and is lowered to "run drop glue now, then mark moved" so a later automatic drop
is suppressed (no double free).

### Decision 2 — Drop-glue insertion points

Drop glue for an owning local runs when the local goes out of scope and still
owns its value. Insertion covers:

- normal block exit (reverse declaration order),
- early `return`, `?` propagation, `break`, `continue`,
- the unwind/abort path (best-effort release, no re-entrant unwinding),
- partial moves: a field moved out is not dropped; the remaining fields are.

Conditionally-initialized locals use drop flags (a hidden bool per fallible
local) so a value initialized on only one branch is dropped on exactly the paths
where it is live.

### Decision 2a - Backend scope for P0 drop glue

This change's codegen requirement is the production LLVM-text path used by
`sgc` native builds plus the compiler crate's `JITCodegen` LLVM-text emitter.
The `sgc` Cranelift fast-JIT is intentionally not treated as a general MIR
backend here: it evaluates a constant-expression subset and emits a trivial
Cranelift `main`, so it has no function calls, aggregates, ownership, or drop
scope machinery to lower. A future real Cranelift MIR backend must add its own
drop-glue conformance tests before claiming parity.

### Decision 3 — Move and use-after-move checking

A non-`Copy` value is moved when passed by value, returned, or assigned. After a
move the source local is dead; any later read is a compile error
`use-after-move` (stable code, surfaced in `sgc --error-format json` and
`sglsp`). `Copy` types (the integer/bool/float scalars and `&T`) are never moved.

### Decision 4 — Migrating runtime handles

Each owning handle type (`Buffer`, `Vec<T>`, `String`, `JsonDoc`, `ProcessHandle`,
net handles) gains a compiler-known `Drop` impl that calls its existing C free
function. The Sengoo wrapper keeps its `free()/drop()/close()` method for
source compatibility, re-implemented as "explicit early drop". The runtime free
functions become idempotent (guard against a zero/again handle) so the
"explicit then automatic" sequence is safe.

### Decision 5 — `Rc<T>` opt-in

`Rc<T>` is a library type with non-atomic refcounting and `clone`/`Drop`. It is
the escape hatch for shared ownership and for breaking ownership trees; cycles
through `Rc` leak by definition and that is documented. `Arc<T>` (atomic) is
specified in `concurrency-safety-and-async-io`.

`Rc<T>` SHALL use one representation for every monomorphized payload rather
than storing a second `marker: T` value in each handle:

```sg
struct Rc<T> {
    handle: i64,
}

impl<T> Rc<T> {
    def new(value: T) -> Rc<T>;
    def clone(&self) -> Rc<T>;
    def borrow(&self) -> &T;
    def strong_count(&self) -> i64;
}
```

The compiler lowers `Rc<T>::new` with the concrete size/alignment of `T` and a
per-`T` drop thunk. The runtime control block copies the moved payload into
aligned storage and records that thunk. The final `Rc<T>` release invokes the
thunk exactly once and then frees both payload storage and the control block.
`borrow()` returns a reference into that storage; normal lexical borrow checks
therefore prevent moving/dropping the last `Rc<T>` while the reference is live.
Until associated constructors are stabilized, the implemented source spelling is
`rc_new<T>(value)` for generic payloads plus the existing `rc_new_i64`,
`rc_new_bool`, and `rc_new_string` compatibility constructors. Value-returning
`get()` helpers remain scalar/string conveniences, while generic
`borrow() -> &T` is the remaining API slice for this decision.

## Risks / Trade-offs

- **Unwind path complexity.** Mitigation: start with abort-on-panic semantics
  (drop glue still runs on normal/early-exit paths), and only add unwinding drop
  if/when panics become recoverable.
- **Existing examples double-release.** Mitigation: idempotent free + "explicit
  drop marks moved"; a conformance test runs the old manual-free examples and
  the new auto-drop examples side by side.
- **Drop order surprises.** Mitigation: document reverse-declaration order and
  test it.

## Migration

Additive: new code omits `.free()/.drop()/.close()`; old code keeps working. A
follow-up change (out of scope here) may lint or deprecate explicit release once
the realworld fixtures and flagship app are migrated.
