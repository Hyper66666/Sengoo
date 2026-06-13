## Context

The compiler has generic-typeck, HIR specialization, and `mir_generic_methods`
test lanes, and an interned-types baseline (`openspec/specs/interned-types`).
This change turns those building blocks into a complete, user-facing generic +
trait system. Monomorphization is the agreed strategy (umbrella `design.md`
Decision 2).

## Goals / Non-goals

- Goal: generic libraries and a core trait set usable across arbitrary types.
- Goal: both static (monomorphized) and dynamic (`dyn`) dispatch.
- Non-goal: HKT, GATs, const generics, specialization, full coherence.

## Decisions

### Decision 1 — Monomorphization with deterministic instance naming

Each `(generic item, concrete type-args)` pair produces one specialized MIR
function with a deterministic mangled name (extend the existing
`generic_instances` naming used by `tools/sgc`). Instances are collected by a
reachability walk from roots (`main`, exported FFI, reflected symbols) so unused
instantiations are not emitted (consistent with low-memory pruning).

### Decision 2 — Trait bound checking

A bound `T: Trait` is satisfied if a matching `impl Trait for T` is visible.
Bound checks run during type-check at call/instantiation sites and produce a
stable `unsatisfied-trait-bound` diagnostic naming the trait, the type, and the
missing method(s). `where` clauses desugar to the same bound set.

### Decision 3 — `dyn Trait` vtable ABI

A `dyn Trait` value is a fat pointer `{ data_ptr, vtable_ptr }`. The vtable holds
the trait method pointers plus a `drop` slot and a `size`/`align` pair. Only
*object-safe* traits (no generic methods, no `Self`-by-value returns except via
indirection, no associated-type leakage in the object type) can form `dyn`
objects; non-object-safe use is a stable diagnostic.

### Decision 4 — Associated types

Traits may declare `type Item;`. Impls must define each associated type. In
generic code, associated types are referenced as `T::Item` and resolved during
monomorphization; in `dyn` objects associated types must be fixed by the object
type (`dyn Iterator<Item = i64>`).

### Decision 5 — Core trait set and derive

The core traits are compiler-known so codegen and stdlib can rely on them:

| Trait | Methods (essential) | Derivable |
| --- | --- | --- |
| `Clone` | `clone(&self) -> Self` | yes |
| `Copy` | marker (implies bitwise copy, no `Drop`) | yes |
| `PartialEq`/`Eq` | `eq(&self, other: &Self) -> bool` | yes |
| `PartialOrd`/`Ord` | `cmp(&self, other: &Self) -> Ordering` | yes |
| `Hash` | `hash<H: Hasher>(&self, h: &mut H)` | yes |
| `Default` | `default() -> Self` | yes |
| `Display` | `fmt(&self, f: &mut Formatter) -> Result` | no (user intent) |
| `Debug` | `fmt(&self, f: &mut Formatter) -> Result` | yes |
| `Iterator` | `type Item; next(&mut self) -> Option<Item>` | no |
| `IntoIterator` | `type Item; type IntoIter; into_iter(self)` | no |

`#[derive(...)]` reuses the existing derive-macro expander
(`compiler/src/parser/derive_expander.rs`).

### Decision 6 — Orphan rule

An `impl Trait for Type` is allowed only if either the trait or the type is
defined in the current package. This prevents conflicting downstream impls
without a full coherence solver.

## Risks / Trade-offs

- **Monomorphization code size.** Mitigation: reachability-based instance
  collection + dedup; `dyn` available where binary size matters.
- **Object safety confusion.** Mitigation: precise diagnostic explaining which
  rule failed.
- **Interaction with `Drop`/`Copy`.** Mitigation: `Copy` and `Drop` are mutually
  exclusive (checked); coordinate with `automatic-memory-management`.

## Migration

The stdlib `Option<T>`/`Result<T, E>` and collections move to generic impls in
follow-up work (`generic-collections`). Existing scalar helper names remain
during the transition (umbrella Decision 5).
