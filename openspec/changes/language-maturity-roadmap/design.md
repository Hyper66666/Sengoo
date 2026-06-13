## Context

This umbrella coordinates the language-level maturity work (P0–P2) identified
after `six-pillar-gap-closure`. The six-pillar program closed *workflow* and
*stdlib-surface* gaps; this program closes *language-semantics* and *ecosystem*
gaps. The constraints below are intentionally frozen so child changes share one
contract.

## Goals / Non-goals

- Goal: make idiomatic Sengoo code memory-safe by default, generic and
  reusable, and ergonomic for text — without a manual-free style.
- Goal: keep every transition source-compatible; no flag day.
- Non-goal: a garbage collector (see Decision 1), self-hosting, or breaking the
  existing handle-based stdlib.

## Decisions

### Decision 1 — Memory model: ownership + automatic `Drop`, not RC + cycle GC

The original spec (`Sengoo_Language_Specification.md` §1.3) proposed RC + cycle
GC. We instead standardize on **move-based ownership with compiler-inserted
`Drop`** (RAII), because:

- The compiler already has a borrow checker (`compiler/src/typeck/borrow.rs`)
  and MIR; ownership + `Drop` is the natural completion of that design.
- It keeps the "C/Rust-class runtime performance" claim in the README intact
  (no GC pauses, no RC atomics on every handle).
- It gives deterministic resource release for FFI handles, files, and sockets,
  which the manual `.close()` APIs already assume.

Consequence: `automatic-memory-management` owns the `Drop` trait, drop-glue
insertion in MIR, move/use-after-move checking, and migrating runtime handles
(`Buffer`, `Vec`, `String`, `JsonDoc`, process/net handles) to auto-drop. RC
(`Rc<T>`/`Arc<T>`) is offered as an *opt-in library type* for shared ownership,
not the default.

### Decision 2 — Generics are monomorphized, with `dyn` for dynamic dispatch

Generic functions/methods/types are monomorphized per instantiation (consistent
with the existing `mir_generic_methods` / `hir_specialization` test lanes).
Dynamic dispatch is opt-in through `dyn Trait` trait objects with a vtable ABI.
This lets the stdlib drop scalar hand-specialization.

### Decision 3 — Core trait set is the contract between language and stdlib

A fixed core trait set (`Clone`, `Copy`, `Drop`, `Eq`/`PartialEq`,
`Ord`/`PartialOrd`, `Hash`, `Default`, `Display`, `Debug`, `Iterator`,
`IntoIterator`) is defined once in `generics-and-trait-system` and consumed by
`generic-collections`, `strings-and-formatting`, and `numeric-type-system`.
`#[derive(...)]` is supported for the obvious ones (the repo already has a
`derive_macro` test lane to build on).

### Decision 4 — Strings build on the memory model

Owned `String` becomes a normal move-only value with `Drop` (no manual
`.drop()`), `&str` is a borrowed view, formatting is trait-based (`Display`),
and `print`/`println` accept any `Display`. Interpolation `f"{x}"` lowers to
`format` calls. This depends on Decisions 1 and 3.

### Decision 5 — Transition strategy: additive, dual-surface

Every child adds safe APIs next to existing handle APIs. Old names remain
source-compatible. A later, separate change proposes deprecations once the safe
surface is proven by the realworld fixtures and the flagship app.

## Cross-pillar dependency graph

```
automatic-memory-management ─┬─> first-class-strings-and-formatting
                             ├─> generic-collections
generics-and-trait-system ───┼─> generic-collections
                             ├─> first-class-strings-and-formatting
                             └─> numeric-type-system (core numeric traits)
generic-collections ─────────> (stdlib rewrite, separate follow-up)
concurrency-safety-and-async-io ── depends on memory model (Send/Sync on Drop types)
debugger-and-test-framework ── independent, can start early
package-registry-and-distribution ── independent (P2)
wasm-and-bytecode-backends ── depends on stable MIR/runtime ABI
authoritative-language-reference ── tracks all of the above; archived last
flagship-reference-application ── consumes P0+P1; archived near the end
```

## Risks / Trade-offs

- **Scope.** This is a multi-quarter program. Mitigation: strict child-change
  isolation; the umbrella only archives after all required children archive.
- **Ownership migration churn.** Auto-`Drop` changes how existing examples free
  resources. Mitigation: dual-surface transition (Decision 5) and a conformance
  gate that runs both old and new examples.
- **ABI stability for backends.** WASM/bytecode depend on a stable MIR/runtime
  ABI; start them only after P0 lands.

## Open questions

- Exact `format` mini-language (subset of Rust's `{}` / `{:?}` / width/precision)
  is decided inside `strings-and-formatting`.
- Whether `Arc<T>` + threads or a higher-level structured-concurrency API is the
  primary multi-thread surface is decided inside `concurrency-safety-and-async-io`.
