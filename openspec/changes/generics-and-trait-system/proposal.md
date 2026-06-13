## Why

Generics and traits are too weak to write reusable code, which forces the
standard library to hand-specialize per scalar type:

- `std::collections` ships `Vec<i64>`, `Vec<bool>`, `StringMapI64`,
  `StringMapBool`, `TextList` — no general `Vec<T>` or `HashMap<K, V>`.
- `examples/generics/03_result_chain.sg` re-declares `struct Result<T, E>` and
  only `impl Result<i64, i64>` (a concrete impl); generic methods over arbitrary
  `T` are not usable.
- `examples/traits/01_iterator_basic.sg` shows a trait that is only a default
  method on a concrete type; there are no trait bounds, no trait objects, and no
  core traits (`Clone`, `Eq`, `Ord`, `Hash`, `Display`, `Iterator`).

Without a real generic + trait system, users cannot write libraries and the
stdlib cannot stop duplicating scalar variants. This is a hard prerequisite for
`generic-collections`, `numeric-type-system`, and `strings-and-formatting`.

The compiler already has the scaffolding to build on: `generic_typeck`,
`generic_constraints`, `generic_hir`, `mir_generic_methods`,
`hir_specialization`, `trait_dispatch`, and `derive_macro` test lanes exist.

## Proposal

Deliver a monomorphization-based generic system with a real trait layer.

- **Generic functions, methods, and types** over arbitrary type parameters,
  monomorphized per instantiation (one specialized MIR body per concrete set of
  type arguments), with deterministic instance naming.
- **Trait bounds**: `def f<T: Trait>(...)`, `where` clauses, and bound checking
  at the call site with a stable "unsatisfied bound" diagnostic.
- **Trait objects** `dyn Trait` with a vtable ABI for dynamic dispatch, alongside
  the static (monomorphized) path.
- **Associated types** on traits (`type Item;`) so `Iterator` and friends can be
  expressed.
- **A fixed core trait set**: `Clone`, `Copy`, `Eq`/`PartialEq`,
  `Ord`/`PartialOrd`, `Hash`, `Default`, `Display`, `Debug`, `Iterator`,
  `IntoIterator` (and `Drop` from `automatic-memory-management`).
- **`#[derive(...)]`** for `Clone`, `Copy`, `Eq`/`PartialEq`, `Ord`/`PartialOrd`,
  `Hash`, `Debug`, `Default`, building on the existing derive-macro lane.

## What changes

- ADDED: generic functions/methods/impls with type-parameter monomorphization.
- ADDED: trait bounds + `where` clauses + bound checking diagnostics.
- ADDED: `dyn Trait` trait objects and vtable dispatch.
- ADDED: associated types.
- ADDED: the core trait set and `#[derive]` support for it.

## Non-goals

- Higher-kinded types, generic associated types (GATs), const generics, or
  specialization. These can be proposed later if needed.
- Trait coherence beyond a basic orphan rule (a pragmatic orphan rule is
  specified; full coherence theory is out of scope).
