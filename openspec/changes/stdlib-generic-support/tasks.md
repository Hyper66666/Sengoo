## 1. Baseline Inventory

- [x] 1.1 Audit the current stdlib generic declarations in `tools/stdlib/collections.sg`.
- [x] 1.2 Audit existing compiler and `sgc` tests that already cover stdlib generic instantiations.
- [x] 1.3 Record the actual shipped boundary: generic source surface vs runtime-operational support.

## 2. Option / Result Hardening

- [x] 2.1 Keep `Option<T>` and `Result<T, E>` on the current tagged-struct representation for this phase, backed by the tagged-struct layout regression.
- [x] 2.2 Verify method specialization for `unwrap_or`, `ok`, `err`, and `ok_or` across mixed concrete instantiations.
- [x] 2.3 Add or tighten cross-module tests for the current `Option<T>` / `Result<T, E>` boundary, including the present import/type-resolution limitation.

## 3. Vec / HashMap Boundary Clarification

- [x] 3.1 Verify that generic handle-shell methods compile for non-`i64` instantiations.
- [x] 3.2 Document that mutating and lookup runtime operations remain specialized to the current helper family.
- [x] 3.3 Add tests that distinguish generic type-surface support from runtime-operational support.

## 4. Codegen and Cache Verification

- [x] 4.1 Verify repeated-build reuse through the existing `sgc` generic cache path for stdlib generic impl-method instantiations.
- [x] 4.2 Reframe cross-module stdlib generic verification around the current imported-type boundary instead of claiming positive imported-type monomorphized codegen.
- [x] 4.3 Confirm no regressions in existing `generic_typeck`, compiler `stdlib_surface_`, and `sgc` `stdlib_surface_runtime_` suites.

## 5. Follow-up Capture

- [x] 5.1 Add a design note that defers true generic container runtime support to a separate phase-B change.
- [x] 5.2 Explicitly list unresolved decisions for `Vec<T>` / `HashMap<K, V>` runtime strategy.
