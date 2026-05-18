## ADDED Requirements

### Requirement: Stdlib generic surface SHALL match current shipped behavior
The stdlib generic proposal SHALL describe the generic types and methods that
already exist in the repository instead of treating stdlib generics as
unimplemented from scratch.

#### Scenario: Existing stdlib generic declarations are part of the baseline
- **WHEN** maintainers review the stdlib generic support change
- **THEN** the baseline includes `Option<T>`, `Result<T, E>`, `Vec<T>`, and
  `HashMap<K, V>` declarations already present in `tools/stdlib/collections.sg`

### Requirement: `Option<T>` and `Result<T, E>` SHALL remain tagged-struct based in this phase
This change SHALL preserve the current tagged-struct representation for
`Option<T>` and `Result<T, E>` and treat enum migration as separate work.

#### Scenario: Tagged-struct layout remains the locked baseline
- **WHEN** maintainers validate the current `Option<T>` / `Result<T, E>`
  representation for this change
- **THEN** the change artifacts reference the tagged-struct layout regression as
  the evidence that phase 2.1 is complete

#### Scenario: Mixed `Option<T>` / `Result<T, E>` instantiations monomorphize
- **WHEN** compiler tests instantiate `Option<bool>`, `Option<i64>`,
  `Result<bool, i64>`, or `Result<i64, bool>`
- **THEN** code generation emits distinct specialized instances without
  unresolved generic leakage

### Requirement: Container generic surface SHALL be distinguished from runtime-operational support
The stdlib generic proposal SHALL distinguish generic type-surface support for
`Vec<T>` / `HashMap<K, V>` from the narrower set of runtime-operational methods
that are currently backed by specialized helpers.

#### Scenario: Generic handle-shell container types compile beyond `i64`
- **WHEN** source uses shared-handle or non-mutating generic container methods
  on types such as `Vec<bool>` or `HashMap<bool, bool>`
- **THEN** the compiler accepts those instantiations as part of the shipped
  stdlib generic surface

#### Scenario: Runtime-operational container methods remain specialized
- **WHEN** maintainers assess container runtime support in this change
- **THEN** the proposal and tasks describe full generic container runtime as
  deferred follow-up work instead of claiming it is already implemented

#### Scenario: Current container runtime boundary is documented explicitly
- **WHEN** maintainers read the stdlib generic surface change artifacts
- **THEN** the artifacts state that shared-handle methods are available on the
  generic container shell while mutating and lookup operations still depend on
  the current specialized helper family

#### Scenario: Cross-file imported stdlib generic type names remain outside the current boundary
- **WHEN** source relies on file imports to expose stdlib generic type names
  such as `Option<T>` in another module
- **THEN** the change artifacts describe that limitation explicitly instead of
  claiming cross-file imported stdlib generic monomorphization already works

#### Scenario: Repeated-build cache reuse covers generic impl methods
- **WHEN** maintainers review repeated-build stdlib generic cache verification
- **THEN** the change artifacts reference `sgc` tests proving typed-receiver
  and chained method-return receiver impl-method instantiations enter
  generic-instance planning and are reused on a warm cache

### Requirement: Stdlib generic baseline SHALL be backed by compiler and runtime evidence
The baseline for this change SHALL be justified by existing compiler and `sgc`
tests rather than proposal-only claims.

#### Scenario: Existing stdlib generic suites pass
- **WHEN** targeted stdlib generic verification runs are executed
- **THEN** the `generic_typeck` suite, compiler-side `stdlib_surface_` suite,
  and `sgc` `stdlib_surface_runtime_` suite pass and serve as baseline evidence
  for the change
