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
