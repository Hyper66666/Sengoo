# Proposal: Stdlib Generic Surface Consolidation

## Status

**Proposed** - artifacts aligned to current workspace evidence; repeated-build
cache verification for stdlib generic impl methods is now covered by the
current `sgc` generic-instance planning path.

## Summary

Consolidate and formalize the generic standard-library surface that already
exists in the repository, then stage true generic container runtime support as
follow-up work instead of pretending it is already one change away.

This proposal does **not** assume stdlib generics are starting from zero.
`Option<T>`, `Result<T, E>`, `Vec<T>`, and `HashMap<K, V>` already exist as
generic stdlib-facing types in `tools/stdlib/collections.sg`, and current
compiler tests already verify multiple monomorphized instantiations.

The real gap is narrower and more important:

- document the shipped generic boundary accurately
- keep `Option` / `Result` stable and verified as generic tagged structs in
  this phase
- make the `Vec` / `HashMap` runtime boundary explicit instead of calling them
  "fully generic" when operational methods are still backed by `i64`-specific
  runtime helpers
- define a clean phase-B lane for true generic container runtime support

## Why

The current proposal shape overstates what is missing and understates the real
implementation risk.

Today the repository already has:

- compiler support for generic functions, structs, where clauses, default type
  arguments, and generic aliases
- generic stdlib declarations in `tools/stdlib/collections.sg`
- stdlib surface tests covering `Option<bool>`, `Option<i64>`,
  `Result<bool, i64>`, `Result<i64, bool>`, `Vec<bool>`, `Vec<i64>`,
  `HashMap<bool, bool>`, and `HashMap<i64, i64>`
- method specialization tests showing monomorphized IR for stdlib generic
  methods
- a tagged-struct layout regression that now locks the current `Option<T>` /
  `Result<T, E>` representation

What it does **not** yet have is a fully generic runtime container model for
`Vec<T>` and `HashMap<K, V>`. Those types currently expose a generic source
surface but route operational methods through `i64`-specialized FFI helpers.

If we keep calling the missing work "enable generic stdlib support", the
implementation will waste time re-auditing parser and type-check paths that are
already working, while still failing to make the container-runtime boundary
explicit.

## Scope

This change should cover:

- inventorying and documenting the generic stdlib surface that already ships
- verifying and hardening `Option<T>` / `Result<T, E>` generic behavior
- clarifying the current `Vec<T>` / `HashMap<K, V>` runtime contract
- recording the current imported-type boundary truthfully in cross-module
  verification
- verifying repeated-build reuse for stdlib generic impl-method
  instantiations through `sgc` generic-instance planning
- recording phase-B follow-up work for true generic container runtime support

This change should **not** cover:

- rewriting `Option<T>` / `Result<T, E>` from tagged structs to enums
- introducing `None` / `Some` / `Ok` / `Err` language-level enum constructors
  as part of this proposal
- replacing handle-backed containers with inline `ptr/len/cap` storage in the
  same change
- generalized drop/destructor semantics for arbitrary `T`
- full generic runtime FFI for every container operation

## Current State

Evidence in the repository already shows partial stdlib generic support:

- `tools/stdlib/collections.sg` defines:
  - `struct Option<T>`
  - `struct Result<T, E>`
  - `struct Vec<T>`
  - `struct HashMap<K, V>`
- `compiler/src/tests/stdlib_surface_tests.rs` already verifies:
  - mixed `Option<T>` and `Result<T, E>` monomorphization
  - generic stdlib methods returning correct LLVM types
  - stdlib generic types across multiple concrete instantiations
- `tools/sgc/src/tests.rs` includes runtime-level stdlib surface coverage
- the current workspace state also includes passing `generic_typeck`,
  compiler `stdlib_surface_`, and `sgc` `stdlib_surface_runtime_` suites

So the correct framing is:

- `Option<T>` / `Result<T, E>`: generic surface already exists and should be
  hardened
- `Vec<T>` / `HashMap<K, V>`: generic shell exists, but runtime behavior is
  only fully operational for the currently specialized `i64` helper family

More concretely:

- shared-handle and shape-preserving methods such as `len()`, `is_empty()`,
  `clear()`, and `free()` already operate through the generic handle shell
- cross-file import graphs are present in the build pipeline, but imported
  stdlib generic type names are not yet a supported type-resolution surface
- repeated-build reuse for stdlib generic impl methods is now verified through
  the `sgc` generic-instance cache path for typed-receiver and chained
  method-return receiver call sites
- mutating and lookup operations such as `push`, `pop`, `get`, `set`,
  `remove`, `contains`, and iterator item retrieval are still routed through
  `sengoo_*_i64` helper families in the current stdlib/runtime surface

## Proposal

Split the work into two layers.

### Layer A: Formalize shipped stdlib generics

This change covers layer A.

- Keep `Option<T>` and `Result<T, E>` in their current tagged-struct form
  during this phase.
- Treat parser and type-check support as an audit item, not the core work.
- Add tests and docs that distinguish:
  - generic source-level declarations
  - monomorphized method/codegen support
  - runtime-operational container support
- Make the current `Vec<T>` / `HashMap<K, V>` boundary explicit:
  - handle-based representation is generic at the type surface
  - operational runtime helpers remain specialized for the current helper
    family (`vec_new_i64`, `hashmap_new_i64_i64`, `sengoo_vec_*_i64`,
    `sengoo_hashmap_*_i64`)
- Treat repeated-build cache reuse as a verified Layer A item for stdlib
  generic impl-method instantiations that participate in `sgc`
  generic-instance planning.

### Layer B: True generic container runtime

This is follow-up work, not part of the current change.

That phase must make an explicit design choice between:

- typed runtime helper families
- erased/boxed container runtime with compiler-managed casting
- fully monomorphized inline container layouts

It must also answer at least these unresolved questions explicitly:

- whether `Vec<T>` / `HashMap<K, V>` stay handle-backed or become inline data
  structures
- how arbitrary element/key/value storage is represented at runtime
- whether drop/destructor semantics are introduced as part of that phase or
  remain unsupported
- what ABI and caching rules apply once container operations become truly
  generic

That is a larger architectural decision and should not be hidden inside a
"stdlib generics" proposal that also claims parser work and `Option<T>`
verification.

## Risks

- The biggest risk is spec drift: calling `Vec<T>` and `HashMap<K, V>` "fully
  generic" when the runtime path is still specialized.
- Generic cache and monomorphization behavior may still have edge cases across
  stdlib module boundaries, even though typed-receiver and chained
  method-return receiver impl-method cache reuse is now covered.
- Async and stdlib generics may interact through monomorphized method codegen,
  so at least one repeated-build or async-adjacent smoke check should remain in
  scope.

## Effort Shape

This proposal should be treated as a smaller consolidation change, not a full
stdlib-runtime rewrite.

- Layer A: roughly 16-28 hours
- Layer B: separate change, likely much larger depending on runtime strategy

## Recommendation

Adopt this proposal only after rewriting the old "full generic stdlib support"
framing into the narrower and more truthful plan above. The main value is not
new syntax. The main value is removing ambiguity about what stdlib generics
already support and what still requires dedicated runtime work.
