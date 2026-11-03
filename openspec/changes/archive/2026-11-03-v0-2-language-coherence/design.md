## Frozen semantics

### D1: Borrow regions end at last reachable use

Borrow checking remains intraprocedural and inference-only. A local borrow is
live from creation through its last reachable use, not automatically to block
end. Moving or mutating the owner is allowed after that point. Returning a
reference is allowed only when it is derived from an input reference and the
returned path cannot reference a local, temporary, or owned-by-value parameter.

### D2: Move paths are field-sensitive for named aggregates

Moving `value.field` invalidates that owning field while preserving other
independent fields. Reading the whole aggregate or the moved field is rejected.
Drop glue drops only initialized, not-moved owning paths in reverse structural
order. Union-like overlap and arbitrary indexed partial moves remain unsupported.

### D3: Temporary cleanup occurs at the full-expression boundary

An owning temporary that is not moved into a longer-lived owner is dropped at
the end of its full expression. Temporaries extended by a `let` initializer live
as long as the binding required by the inferred borrow. Structured exits
(`return`, `?`, `break`, `continue`) drop all live owning paths exactly once.
Panics do not promise stack unwinding in v0.2.

### D4: Match coverage is structural and guards do not prove coverage

- Enum and bool matches must be exhaustive.
- Integer, char, string, and open-ended domains require `_` for exhaustiveness.
- A guarded arm does not remove a constructor from the remaining coverage set.
- Arms after a complete unguarded coverage set are rejected as unreachable.
- By-value payload bindings move non-Copy fields; `ref`/reference patterns borrow
  when the accepted parser syntax identifies a borrowed payload.

### D5: Receiver-less trait methods use existing path syntax

A trait may declare a method without `self`. Calls use `Trait::method(args)` or
`Type::method(args)`. Argument and expected-result types must select exactly one
impl; otherwise type checking reports `ambiguous-trait-associated-function`.
No new `<Type as Trait>::method` grammar is introduced in this wave.

`Self::Assoc` and `T::Assoc` projections must resolve in trait declarations,
impls, generic signatures, return types, local annotations, and nested generic
arguments. Unbound or ambiguous projections use stable diagnostics.

### D6: Fixed arrays are owning aggregates

Fixed arrays have compile-time length, bounds-checked indexing, deterministic
left-to-right iteration, and element-wise Drop in reverse index order. Moving a
non-Copy array moves the whole value. Indexed partial moves are rejected in v0.2.

## Diagnostics

The following categories must be stable across text, JSON, and LSP output:

- `use-after-partial-move`
- `cannot-move-borrowed`
- `borrow-escapes-owner`
- `non-exhaustive-match`
- `unreachable-match-arm`
- `ambiguous-trait-associated-function`
- `unresolved-associated-type`
- `array-index-out-of-bounds` for compile-time-known invalid indices

Existing compatible codes may be reused if the canonical diagnostic table is
updated before implementation.
