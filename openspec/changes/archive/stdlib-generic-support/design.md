## Context

The repository already ships a partial stdlib generic surface:

- `Option<T>` and `Result<T, E>` exist as tagged structs
- `Vec<T>` and `HashMap<K, V>` exist as generic handle-shell container types
- compiler tests already verify mixed stdlib monomorphization behavior

That means the problem is no longer "how do we enable stdlib generics at all?"
The real design problem is how to describe and harden the current boundary
without lying about container runtime support.

## Decisions

1. Treat current repository state as the baseline
- Proposal and tasks must start from the fact that stdlib generics already
  exist in source and tests.
- Audit is still useful, but it is not license to ignore current evidence.

2. Keep `Option<T>` / `Result<T, E>` on tagged-struct representation in this phase
- The current stdlib and tests already assume:
  - `Option<T> { is_some: bool, value: T }`
  - `Result<T, E> { is_ok: bool, value: T, error: E }`
- Rewriting them to enums is a separate semantic and layout decision.

3. Separate value-generic support from container-runtime support
- `Option<T>` / `Result<T, E>` already behave like ordinary generic value
  types and are good candidates for hardening in this change.
- `Vec<T>` / `HashMap<K, V>` are different: they expose a generic type surface,
  but current operational methods still rely on specialized runtime helpers.

4. Do not promise fully generic containers in this change
- This change may improve tests, docs, and codegen verification.
- It does not resolve the architectural choice for true generic container
  runtime support.

## Current Boundary

### Already supported

- Generic stdlib type declarations
- Generic stdlib methods with monomorphized specialization
- Tagged-struct layout regression coverage for `Option<T>` / `Result<T, E>`
- Mixed concrete instantiations such as:
  - `Option<bool>`
  - `Option<i64>`
  - `Result<bool, i64>`
  - `Result<i64, bool>`
- Handle-shell container types such as `Vec<bool>` and `HashMap<bool, bool>`
  for non-operational or shared-handle methods
- Shared-handle container operations such as `len()`, `is_empty()`, `clear()`,
  and `free()` through the current handle-backed representation
- Passing `generic_typeck`, compiler `stdlib_surface_`, and `sgc`
  `stdlib_surface_runtime_` suites as the current workspace baseline

### Not yet fully supported

- Truly generic runtime storage and mutation for arbitrary `Vec<T>`
- Truly generic runtime key/value storage for arbitrary `HashMap<K, V>`
- Cross-file imported stdlib generic type names as a supported type-resolution
  surface
- Full element-lifecycle handling for arbitrary `T`
- A final representation decision for generic containers

### Current specialized runtime boundary

The current stdlib surface mixes generic declarations with specialized runtime
helpers:

- `Vec<T>` / `HashMap<K, V>` are declared generically in stdlib source
- operational helpers such as construction, mutation, lookup, removal, and
  iterator item retrieval still route through `i64`-specific helper families
- this is why `Vec<bool>` and `HashMap<bool, bool>` are real and useful at the
  type surface, but should not be described as having fully generic runtime
  behavior yet
- cross-module verification should stay framed as an imported-type boundary
  limitation, not as positive imported-type monomorphized codegen

## Deferred Follow-up

Phase B should evaluate one of these runtime strategies explicitly:

1. typed runtime helper families
2. erased/boxed runtime container model
3. fully monomorphized inline container layout

That phase should be its own change because it affects runtime layout,
allocation, ownership, and potentially ABI assumptions.

Phase B must also resolve:

- whether handle-backed containers remain the permanent runtime contract
- how non-`i64` element storage is represented and accessed
- whether arbitrary `T` introduces element destruction / ownership semantics
- whether generic instance caching must incorporate container runtime strategy
  into its fingerprints
