## 1. Lift Option and Result Out

- [x] 1.1 Move `struct Option<T>` and `option_some_*` / `option_none_*` helpers from `tools/stdlib/collections.sg` to a new `tools/stdlib/option.sg`.
- [x] 1.2 Move `struct Result<T, E>` and `result_ok_*` / `result_err_*` helpers to a new `tools/stdlib/result.sg`.
- [x] 1.3 Verify `cargo test -p sengoo-compiler stdlib_surface_` still passes after the move (the test file may need an `import "option"` / `import "result"` line if previously implicit).

## 2. Trim collections.sg

- [x] 2.1 Reduce `tools/stdlib/collections.sg` to: `extern "C" { ... }` block, `struct Vec<T>` / `VecIter<T>`, `struct HashMap<K, V>` / `HashMapIter<V>`, `trait Iterator`, and their `_i64`-specialized helpers and impl blocks.
- [x] 2.2 If `collections.sg` references `Option<T>` or `Result<T, E>` after lift-out, add explicit `import "option"` / `import "result"` lines and verify resolution.

## 3. Add String Module

- [x] 3.1 Confirm runtime helpers `sengoo_str_len`, `sengoo_str_eq`, `sengoo_str_concat` exist in `runtime/src/reflect.rs` (or wherever string FFI lives).
- [x] 3.2 If any of those are missing, add them as a prerequisite commit before this change goes in. Out of scope: floating-point or wide-char string ops.
- [x] 3.3 Create `tools/stdlib/string.sg` with Sengoo-side `str_len`, `str_eq`, `str_concat` wrappers. Current `&str` is intentionally routed through built-in string lowering because Sengoo FFI rejects reference types as not FFI-safe.

## 4. Add Math Module

- [x] 4.1 Create `tools/stdlib/math.sg` with pure-Sengoo `abs_i64`, `min_i64`, `max_i64`, `pow_i64`.
- [x] 4.2 No extern declarations in this file.

## 5. Add Error Module

- [x] 5.1 Create `tools/stdlib/error.sg` with pure-Sengoo `assert(cond: bool)` that calls `sengoo_panic_option_unwrap_i64()` on `false`.
- [x] 5.2 Add `assert_eq_i64(a: i64, b: i64)` helper.
- [x] 5.3 Document that floating-point and string `assert_eq` variants are deferred until the corresponding panic helpers exist.

## 6. Surface Tests

- [x] 6.1 Add per-module smoke tests in `compiler/src/tests/stdlib_surface_tests.rs`:
      - `option_module_imports_and_unwraps`
      - `result_module_imports_and_chains`
      - `string_module_imports_and_runs_str_len`
      - `math_module_imports_and_runs_abs_i64`
      - `error_module_imports_and_asserts_true`
- [x] 6.2 Existing collections tests continue to pass without modification.

## 7. Documentation

- [x] 7.1 Add `tools/stdlib/README.md` listing each module file with a one-paragraph surface description.
- [x] 7.2 Cross-reference the README from the main `README.md` and `README.zh-CN.md` "Standard Library" section (add the section if it does not exist yet).

## 8. Verification

- [x] 8.1 `cargo test -p sengoo-compiler --lib` stays green.
- [x] 8.2 `cargo test -p sgc stdlib_surface_runtime_` stays green.
- [ ] 8.3 `cargo build --workspace` stays green. Blocked on 2026-05-12: `cargo build --workspace` cannot write `target/debug/.fingerprint/.../bin-emit_ir` in this environment (os error 5), and escalation was unavailable.
- [x] 8.4 No new symbols exported from `runtime/src/reflect.rs`; this is a pure source-side reorganization.
