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
- [x] 3.2 Fat-pointer `{ data, vtable }` representation; vtable with method
  slots + `drop` + size/align.
  - `%__dyn_Trait = { i8*, i8* }` fat pointer; one `[N x i64]` vtable
    global per `(trait, concrete)` pair with deterministic prefix slots:
    `drop`, size, align, then method slots.
  - Per-vtable erased drop thunks call the concrete `Drop` impl when it
    exists and use no-op thunks otherwise.
  - Owned `dyn Trait` bindings now lower scope-exit and explicit early drop
    through the per-trait `__dyn_Trait_Drop_drop` helper (vtable drop slot,
    null/no-op guarded), with explicit drop suppressing the scope-exit drop.
- [x] 3.3 Dynamic dispatch codegen in the LLVM-text and Cranelift paths.
  - Done (LLVM-text): `&Concrete -> &dyn Trait` coercion, by-pointer dispatch
    shims, vtable-slot load + `CallIndirect` for single-trait `&self` methods.
  - Done (JIT LLVM-like path): emits dyn vtable globals, struct fat-pointer
    aggregates/extracts, and `CallIndirect` lowering for single-trait `&self`
    dispatch.
  - Done: `&mut self` receivers dispatch through the same fat-pointer path in
    the LLVM-text/JIT text lanes, and unsupported `dyn A + B` / `Box<dyn Trait>`
    forms now report stable diagnostics instead of internal errors.
  - Out of scope here (tracked as stable diagnostics or future changes):
    native Cranelift path if re-enabled; multi-trait `dyn A + B`; value
    receivers; `Box<dyn Trait>`.
- [x] 3.4 Tests: `dyn Trait` call dispatches to the concrete impl; dropping a
  `dyn` value runs the concrete `Drop`.
  - Done: IR-level dispatch tests (`tests::dyn_dispatch_tests`) +
    `examples/traits/03_dyn_dispatch.sg` runs and exits 25; JIT codegen
    regression covers vtable emission and `inttoptr` indirect-call lowering.
  - Done: IR-level tests cover vtable drop thunk calls, no-op drop slots,
    `&mut self` dyn dispatch, and stable diagnostic codes in compiler, `sgc`
    JSON, and `sglsp`.
  - Native handle-count tests prove scope-exit and explicit early drop of an
    owned `dyn` value each run the concrete `Drop` exactly once.

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
- [x] 5.2 `#[derive(...)]` for Clone, Copy, PartialEq/Eq, PartialOrd/Ord, Hash,
  Debug, Default via the existing derive expander.
  - Builtin derive expansion now emits core trait impl declarations for all
    listed derive names so derived types satisfy corresponding generic bounds.
    Verified by `cargo test -p sengoo-compiler
    builtin_derives_register_core_trait_impls_for_bounds -- --nocapture`.
  - Debug now has field-aware formatter behavior for structs and discriminant
    lowering for unit/tuple-payload enums.
  - Clone now generates a real inherent `clone(&self) -> Type` method for
    named structs with scalar/copyable field copies and nested fields whose
    own `clone()` is available, while preserving the `impl Clone for Type {}`
    bound marker. Verified by `cargo test -p sengoo-compiler derive_clone --
    --nocapture`.
  - PartialEq now generates a real inherent `eq(&self, other: &Type) -> bool`
    method for named structs, comparing fields in declaration order; struct
    `==`/`!=` lowering uses that generated method when available. Verified by
    `cargo test -p sengoo-compiler derive_ -- --nocapture`.
  - Default now generates a real inherent `Type::default()` constructor for
    named structs with scalar zero/false field defaults and nested fields whose
    own `Type::default()` is available, while preserving the `impl Default for
    Type {}` bound marker. This also adds the parser/typeck path needed for
    `Type::method()` associated-function calls. Verified by `cargo test -p
    sengoo-compiler derive_default -- --nocapture`.
  - PartialOrd/Ord now generate lexicographic `compare/lt/le/gt/ge` helpers
    for scalar fields and nested fields with ordering operators, and struct
    `< <= > >=` lowering uses the generated `compare` method when available.
    Verified by `cargo test -p sengoo-compiler derive_ord -- --nocapture`.
  - Hash now generates a deterministic `hash() -> i64` helper for scalar fields
    and nested fields whose own `hash()` is available. Verified by
    `cargo test -p sengoo-compiler derive_hash -- --nocapture`.
  - Custom `impl Hash for T` may now define
    `hash_into(&self, h: &mut Hasher)` without spelling `hash()`; the parser
    synthesizes a `hash() -> i64` bridge that drives `hash_into` through a
    fresh stdlib `Hasher`. The stdlib `Hasher` is backed by a native runtime
    byte-state and exposes `write_i64`, `write_bool`, `write_str`,
    `write_string`, and consuming `finish() -> i64`.
  - `#[derive(Hash)]` now routes through a generated
    `hash_into(&self, h: &mut Hasher)` body plus the `hash()` bridge whenever
    a `Hasher` surface is reachable; programs without a hasher keep the
    standalone FNV-1a `hash()` body. A native test proves derived hashes
    match manual `Hasher` writes at runtime.
  - Struct and enum custom `Debug.to_string()` bodies now satisfy the `Debug`
    contract and take precedence over structural `{:?}` formatting; derived
    Debug keeps the built-in structural enum/struct formatting path.
    Follow-ups tracked outside this change: generic collection-field derives
    beyond the currently generated method calls and the general `Formatter`
    object protocol.
- [x] 5.3 Enforce `Copy` and no `Drop`; `Copy` requires all-`Copy` fields.
  - Verified by `cargo test -p sengoo-compiler copy_ -- --nocapture`,
    including `copy-drop-conflict` and `copy-field-not-copy` diagnostics.
- [x] 5.4 Tests for each derive and for the `Copy`/`Drop` exclusivity rule.
  - Completed for the current derive surface: marker impl bounds for all core derives, Copy/Drop exclusivity,
    Copy non-Copy field rejection, Debug struct/unit enum/tuple enum
    formatting, Clone scalar struct copy, PartialEq scalar struct method and
    `==` operator lowering, PartialOrd/Ord scalar struct method and `<`
    operator lowering, Hash scalar struct helper, Default scalar struct
    constructor, plus nested-field Clone/PartialEq/Ord/Hash/Default regressions.
    Custom `hash_into` bridge and stdlib runtime Hasher tests cover the object
    protocol; derived hashes are proven equal to manual `Hasher` writes by a
    native runtime test.

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
- [x] 6.3 Add `examples/generics/` programs using a real generic function with a
  bound and a `dyn Trait` call.
  - Added `examples/generics/05_bound_and_dyn.sg`, which exercises concrete
    instantiation and bound-method dispatch inside a generic function plus a
    `&dyn Shape` dispatch in one runnable program. Verified by `cargo test -p sgc
    examples_smoke_generics_bound_and_dyn -- --nocapture`.
- [x] 6.4 Run `openspec validate generics-and-trait-system --strict`.

## Verification

- `cargo test -p sengoo-compiler --lib` (generic/trait lanes)
- `cargo test -p sgc core_conformance_examples_compile_link_and_run`
- New examples in `examples/generics/` compile, link, and run
