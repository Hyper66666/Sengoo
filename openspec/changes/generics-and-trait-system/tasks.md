## 1. Generic functions, methods, and types

- [x] 1.1 Parse and type-check generic params on `def`, `impl`, `struct`, `enum`
  (extend existing generic-typeck).
- [x] 1.2 Monomorphize each `(item, concrete type-args)` to a specialized MIR
  body with deterministic mangled names (extend `generic_instances`).
- [x] 1.3 Reachability-based instance collection from roots; dedup identical
  instances.
- [x] 1.4 Tests: generic function over two distinct `T`, generic struct +
  generic `impl` methods, generic enum.
  - Verified by `cargo test -p sengoo-compiler generic_ -- --nocapture`
    including generic function, struct, impl method, lazy monomorphization, and
    generic enum payload lowering coverage.

## 2. Trait bounds and where clauses

- [x] 2.1 Resolve `impl Trait for Type` and index visible impls.
- [x] 2.2 Check `T: Trait` / `where` bounds at call and instantiation sites.
- [x] 2.3 Stable `unsatisfied-trait-bound` diagnostic (naming trait, type,
  missing methods) wired into `sgc` JSON and `sglsp`.
- [x] 2.4 Tests: satisfied bound compiles; unsatisfied bound produces the stable
  diagnostic.
  - Verified by compiler, `sgc` JSON, and `sglsp` diagnostic tests for an
    unsatisfied generic function bound. Existing compiler tests cover accepted
    direct and `where` bounds.

## 3. Trait objects (`dyn`)

- [x] 3.0 Parse, lower, and type-check `dyn Trait` / `dyn A + B` as a frontend
  skeleton while vtable/object-safety/codegen remain open.
  - Verified by `cargo test -p sengoo-compiler dyn_trait -- --nocapture`.
- [x] 3.1 Object-safety check with a stable `not-object-safe` diagnostic.
  - Verified by `cargo test -p sengoo-compiler dyn_trait_ -- --nocapture`,
    covering associated functions, generic methods, undefined traits, and
    `Self` returned through reference indirection.
- [~] 3.2 Fat-pointer `{ data, vtable }` representation; vtable with method
  slots + `drop` + size/align.
  - Done: `%__dyn_Trait = { i8*, i8* }` fat pointer; one `[N x i64]` vtable
    global per `(trait, concrete)` pair with deterministic method slots.
  - Deferred: `drop` slot + size/align entries (needs dyn-value Drop).
- [~] 3.3 Dynamic dispatch codegen in the LLVM-text and Cranelift paths.
  - Done (LLVM-text): `&Concrete -> &dyn Trait` coercion, by-pointer dispatch
    shims, vtable-slot load + `CallIndirect` for single-trait `&self` methods.
  - Deferred: Cranelift path; multi-trait `dyn A + B`; value/`&mut self`
    receivers; `Box<dyn Trait>`.
- [~] 3.4 Tests: `dyn Trait` call dispatches to the concrete impl; dropping a
  `dyn` value runs the concrete `Drop`.
  - Done: IR-level dispatch tests (`tests::dyn_dispatch_tests`) +
    `examples/traits/03_dyn_dispatch.sg` runs and exits 25.
  - Deferred: dropping a `dyn` value runs the concrete `Drop`.

## 4. Associated types

- [x] 4.1 Parse `type Item;` in traits and `type Item = ...;` in impls.
  - Verified by `cargo test -p sengoo-compiler associated_type -- --nocapture`;
    impl checking also rejects missing required and unknown associated types.
- [x] 4.2 Resolve `T::Item` in generic code; require fixed associated types in
  `dyn` object types (`dyn Iterator<Item = i64>`).
  - `T::Item` in generic function signatures resolves through the declaring
    trait bound and the concrete impl at call sites; unbounded `T::Item` is
    rejected. Verified by `cargo test -p sengoo-compiler
    associated_type_projection -- --nocapture`.
  - `dyn Trait<Assoc = Type>` parses and type-checks when every required
    associated type is fixed; unfixed associated types use the stable
    `dyn-associated-type` diagnostic. Verified by `cargo test -p
    sengoo-compiler dyn_trait_with_ -- --nocapture`.
- [x] 4.3 Tests covering associated-type resolution and the `dyn` fixing rule.

## 5. Core traits and derive

- [x] 5.1 Define compiler-known core traits (Clone, Copy, PartialEq/Eq,
  PartialOrd/Ord, Hash, Default, Display, Debug, Iterator, IntoIterator) plus
  `Ordering` and `Formatter`/`Hasher` support types.
  - Compiler-known trait names now resolve in bounds, `Iterator`/`IntoIterator`
    include required associated-type names, and support types resolve in
    signatures. Verified by `cargo test -p sengoo-compiler
    compiler_known_core_traits_and_support_types_are_available -- --nocapture`.
  - Behavioral derive impl generation remains in 5.2.
- [ ] 5.2 `#[derive(...)]` for Clone, Copy, PartialEq/Eq, PartialOrd/Ord, Hash,
  Debug, Default via the existing derive expander.
  - Builtin derive expansion now emits core trait impl declarations for all
    listed derive names so derived types satisfy corresponding generic bounds.
    Verified by `cargo test -p sengoo-compiler
    builtin_derives_register_core_trait_impls_for_bounds -- --nocapture`.
  - Remaining: field-aware Clone/compare/hash/debug/default behavior.
- [x] 5.3 Enforce `Copy` and no `Drop`; `Copy` requires all-`Copy` fields.
  - Verified by `cargo test -p sengoo-compiler copy_ -- --nocapture`,
    including `copy-drop-conflict` and `copy-field-not-copy` diagnostics.
- [ ] 5.4 Tests for each derive and for the `Copy`/`Drop` exclusivity rule.
  - Copy/Drop exclusivity and non-Copy field coverage exists; remaining tests
    must cover field-aware behavior for every supported derive.

## 6. Orphan rule and docs

- [x] 6.1 Enforce the package-local orphan rule with a stable diagnostic.
  - Verified by `cargo test -p sengoo-compiler
    orphan_rule_rejects_external_trait_for_external_type -- --nocapture`;
    `cargo test -p sengoo-compiler --lib` confirms local-trait and local-type
    impls still pass.
- [x] 6.2 Document generics, bounds, `dyn`, associated types, and derive in
  `docs/language-features.md`.
  - Documents supported generic bounds, associated types, dyn fixed bindings,
    derive marker impls, Copy/Drop rules, and current dyn/derive limitations.
- [ ] 6.3 Add `examples/generics/` programs using a real generic function with a
  bound and a `dyn Trait` call.
- [x] 6.4 Run `openspec validate generics-and-trait-system --strict`.

## Verification

- `cargo test -p sengoo-compiler --lib` (generic/trait lanes)
- `cargo test -p sgc core_conformance_examples_compile_link_and_run`
- New examples in `examples/generics/` compile, link, and run
