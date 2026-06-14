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
- [ ] 3.1 Object-safety check with a stable `not-object-safe` diagnostic.
- [ ] 3.2 Fat-pointer `{ data, vtable }` representation; vtable with method
  slots + `drop` + size/align.
- [ ] 3.3 Dynamic dispatch codegen in the LLVM-text and Cranelift paths.
- [ ] 3.4 Tests: `dyn Trait` call dispatches to the concrete impl; dropping a
  `dyn` value runs the concrete `Drop`.

## 4. Associated types

- [ ] 4.1 Parse `type Item;` in traits and `type Item = ...;` in impls.
- [ ] 4.2 Resolve `T::Item` in generic code; require fixed associated types in
  `dyn` object types (`dyn Iterator<Item = i64>`).
- [ ] 4.3 Tests covering associated-type resolution and the `dyn` fixing rule.

## 5. Core traits and derive

- [ ] 5.1 Define compiler-known core traits (Clone, Copy, PartialEq/Eq,
  PartialOrd/Ord, Hash, Default, Display, Debug, Iterator, IntoIterator) plus
  `Ordering` and `Formatter`/`Hasher` support types.
  - Partial: `Copy` is now a compiler-known marker trait and its name is
    reserved from user redeclaration with stable `copy-trait-reserved`.
    Remaining core traits and support types are open.
- [ ] 5.2 `#[derive(...)]` for Clone, Copy, PartialEq/Eq, PartialOrd/Ord, Hash,
  Debug, Default via the existing derive expander.
- [ ] 5.3 Enforce `Copy` ⇔ no `Drop` and `Copy` requires all-`Copy` fields.
  - Partial: a type cannot implement both compiler-known `Copy` and `Drop`;
    either source order produces stable `copy-drop-conflict`, including generic
    impl constructors. Recursive all-field `Copy` validation remains open.
- [ ] 5.4 Tests for each derive and for the `Copy`/`Drop` exclusivity rule.
  - Partial: compiler tests cover reserved `Copy` and both declaration orders
    of the `Copy`/`Drop` conflict. Per-derive and non-`Copy` field tests remain
    open.

## 6. Orphan rule and docs

- [ ] 6.1 Enforce the package-local orphan rule with a stable diagnostic.
- [ ] 6.2 Document generics, bounds, `dyn`, associated types, and derive in
  `docs/language-features.md`.
- [ ] 6.3 Add `examples/generics/` programs using a real generic function with a
  bound and a `dyn Trait` call.
- [x] 6.4 Run `openspec validate generics-and-trait-system --strict`.

## Verification

- `cargo test -p sengoo-compiler --lib` (generic/trait lanes)
- `cargo test -p sgc core_conformance_examples_compile_link_and_run`
- New examples in `examples/generics/` compile, link, and run
