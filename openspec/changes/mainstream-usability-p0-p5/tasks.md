## 1. Pinning and prerequisites

- [x] 1.1 Run `openspec validate mainstream-usability-p0-p5 --strict`.
  Passed at proposal time and re-run after each revision.
- [x] 1.2 Record the baseline probe matrix (accepted vs rejected forms) in
  `design.md` so regressions are detectable.
  Recorded in `design.md` Context table.
- [x] 1.3 Confirm `?` type checking needs no change: `peel_option_ty_static` /
  `peel_result_ty_static` match on type name and arity only.
  Confirmed in `try_helpers.rs:35-51` and verified end-to-end: `?` propagates
  user-declared `enum Result`/`enum Option` (probe `chain(10,0)` → 101/1).

## 2. P0 — Option/Result as enums

- [x] 2.1 Define `enum Option<T> { None, Some(T) }` and
  `enum Result<T, E> { Ok(T), Err(E) }` in `tools/stdlib/`; constructors usable
  as values and as patterns.
  Stdlib declarations are the enum form; constructors and `match` work in
  `tools/stdlib/option.sg` and `tools/stdlib/result.sg`.
- [x] 2.2 MIR lowering, layout, and exact-once Drop for enum payloads, including
  `match` arm moves and `?` early return. Extend AMM Drop tests to cover
  `Option`/`Result` payloads before the struct form is removed.
  Verified by the 1136-test compiler library suite, all 36 `drop_flag_tests`,
  and native runtime regressions for conditional match-arm moves, guards,
  `Option`/`Result` scope drops, `unwrap_or`, and `?` early returns.
- [x] 2.3 Compiler-known compatibility accessors (`.is_ok`, `.is_some`,
  `.value`, `.error`) over the enum form, each with a deprecation diagnostic
  naming the pattern replacement.
  Typeck (`compat_enum_field_ty`) and MIR (`lower_compat_enum_field`) resolve
  the four fields on `Option`/`Result` enums only; each hit emits
  `attributes::deprecated_use` with a pattern replacement. Payload reads off
  the other arm yield that type's default value. Proof:
  `compiler/src/tests/compat_enum_field_tests.rs`.
- [x] 2.4 Reject struct-literal construction of these types with a diagnostic
  naming the `Ok`/`Err`/`Some`/`None` replacement.
  Proof: `compiler/src/tests/struct_codegen_tests.rs::{option,result}_struct_literal_diagnostic_names_variant_constructors`.
- [x] 2.5 Deprecate `option_none_with` / `result_*_with` placeholder
  constructors; behavior preserved for the compatibility release.
  Attributes live on the stdlib helpers; proof:
  `compat_enum_field_tests::placeholder_constructors_remain_usable_with_deprecation`.
- [x] 2.6 `?` continues to work unchanged for both types; add regression tests
  for `Result`→`Result`, `Option`→`Option`, and the rejected cross/`main` cases.
  Proof: `try_operator_tests::enum_*_question_*`.
- [x] 2.7 Migrate `tools/stdlib/` (236 field-access sites) to constructors and
  `match`.
- [x] 2.8 Migrate `examples/` and fixtures (195 field-access sites).
- [x] 2.9 Language reference and support matrix updated; `Option`/`Result` rows
  state the enum form and the deprecation window.

## 3. P1 — for over collections

- [x] 3.1 Desugar `for pat in expr` onto the existing `Iterator` protocol for
  collection and iterator receivers.
- [x] 3.2 Keep the current direct lowering for arrays, slices, and ranges; add a
  test proving no iterator indirection is introduced for them.
  Proof: `for_loop_tests::array_for_loop_does_not_lower_through_iterator_next`.
- [x] 3.3 Support `Vec`, `VecDeque`, `HashSet`, `BTreeSet` element iteration and
  `HashMap`/`BTreeMap` entry iteration.
  Proof: `for_loop_tests::for_loop_iterates_vec_and_lazy_adapters` and
  `for_loop_iterates_map_entries_keys_and_values`.
- [x] 3.4 Add `keys()` / `values()` to `HashMap` and `BTreeMap`.
- [x] 3.5 Iterating lazy adapters (`map`, `filter`, `take`, `skip`,
  `enumerate`) with `for` consumes them to completion.
- [x] 3.6 Mutation-while-iterating stays rejected by existing borrow rules;
  regression test included.
  Proof: `for_loop_tests::for_loop_rejects_mutation_while_iterating_a_vec`.

## 4. P2 — Quiet output and one diagnostic language

- [x] 4.1 Move cache, workset, frontend session/scheduler, and generic-instance
  instrumentation behind `--verbose`.
- [x] 4.2 Suppress pass-through toolchain include-path warnings on successful
  builds.
- [x] 4.3 Keep every actionable error and warning at default verbosity.
- [x] 4.4 `--verbose` restores the previous output exactly; test asserts both
  modes.
  Proof: `tools/sgc/tests/quiet_output.rs`.
- [x] 4.5 Translate remaining Chinese compiler diagnostics to English, keeping
  stable codes and JSON shape unchanged.
  Proof: `diagnostics_tests::typeck_diagnostics_are_english_with_stable_wording`.
- [x] 4.6 `--error-format json` payloads unchanged; `sglsp` parity test.
  Proof: `quiet_output::error_format_json_keeps_schema_and_english_messages` and
  `sglsp` `json_error_payload_preserves_stable_code_and_matches_embedded_compiler`.

## 5. P3 — Everyday syntax

- [x] 5.1 `vec![a, b, c]` and `vec![value; count]` as pinned built-in forms.
  Proof: `everyday_syntax_tests::{vec_macro_builds_from_elements,vec_macro_repeat_form_compiles,unknown_bang_form_is_rejected,for_loop_over_vec_macro_compiles}`.
- [x] 5.2 `println` / `print` / `eprintln` accept a format string plus arguments
  through the existing `format` pipeline.
  Proof: `everyday_syntax_tests::println_accepts_format_string_and_arguments`.
- [x] 5.3 `{:?}` renders `#[derive(Debug)]` shapes in format arguments and
  f-string interpolation.
  Proof: `everyday_syntax_tests::debug_placeholder_with_derive_compiles` and
  `parser::fstring::tests::lowers_debug_spec_to_format_placeholder`.
- [x] 5.4 `{:?}` without a `Debug` derive is rejected with a diagnostic naming
  the missing derive.
  Proof: `everyday_syntax_tests::debug_placeholder_without_derive_is_rejected`.
- [x] 5.5 `if let PATTERN = EXPR { .. } else { .. }`, with a diagnostic for
  irrefutable patterns.
  Proof: `everyday_syntax_tests::{if_let_binds_option_payload,if_let_irrefutable_pattern_is_rejected}`.

## 6. P4 — Idiomatic flagship examples

- [x] 6.1 Rewrite `examples/realworld/workspace-audit/src/lib.sg` using early
  `return`, `?`, and flat guard clauses instead of single-line nested
  expressions.
  Proof: `sgpm test --locked --manifest-path examples/realworld/workspace-audit/Sengoo.toml`
  (3 passed, including `test_parallel_score_matches_serial_oracle`).
- [x] 6.2 Rewrite `examples/realworld/cli-json-audit/src/main.sg` the same way.
  Proof: `sgpm test --manifest-path examples/realworld/cli-json-audit/Sengoo.toml`
  (1 passed) and `sgc run src/main.sg` from the package directory exits 0.
- [x] 6.3 Sweep remaining fixtures for collapsed single-line function bodies.
  Do not reformat the repo at large; a width-aware sweep of the other 53
  affected files is a follow-up scheduled after the `Option`/`Result`
  migration.
  Applied width-aware `sgfmt` only to the P4 flagship sources and tests.
  Other realworld packages still contain short inline `if ok { 0 } else { 1 }`
  bodies that fit the default width.
- [x] 6.4 Behavior is unchanged: each rewritten fixture passes its loop with
  identical results. Fixtures holding v1 lockfiles (D8) are verified through
  the non-locked loop and their locked-gate status stays open with the blocker
  recorded, not ticked.
  `cli-json-audit` is verified non-locked. After `4425bba55` the v1 lockfile
  no longer fails `--locked` deserialization, but regenerating those locks
  stays out of scope per D8.

## 7. P5 — Width-aware block formatting

- [ ] 7.1 `format_block_inline` falls back to the multi-line `format_block`
  rendering when the inline form would exceed `max_width`, activating the
  already-parsed but currently unread option. Default stays 100.
- [ ] 7.2 Applies to every block form — `if`, `while`, `for`, `loop`, `match`
  arms, `async`, `parallel`, `try` — not only function bodies.
- [ ] 7.3 Blocks that fit stay inline, unchanged from current behavior.
- [ ] 7.4 `--max-width` and `sgfmt.toml` demonstrably change where blocks break.
- [ ] 7.5 Idiomatic multi-line conditional bodies pass `sgfmt --check`.
- [ ] 7.6 Ships with its own tests, reviewable separately from the P4 rewrites.
- [ ] 7.7 Identify every `fmt --check` gate that newly fails under the rule.
  Do NOT reformat the repo: 53 of 198 in-tree `.sg` files contain 251 lines over
  100 characters, and a sweep would collide with the concurrent `Option`/
  `Result` migration. Apply the new formatting only to fixtures P4 rewrites;
  schedule the sweep as a follow-up after P0 settles.

## 8. Verification

- [ ] 8.1 `cargo fmt --check`
- [ ] 8.2 `cargo test -p sengoo-compiler --lib`
- [ ] 8.3 `cargo test -p sgc`
- [ ] 8.4 `cargo test -p sgpm`
- [ ] 8.5 `cargo test -p sglsp`
- [ ] 8.6 `cargo test -p sgfmt`
- [ ] 8.7 `cargo clippy -p sgc -p sgpm -p sengoo-compiler -p sengoo-runtime -p sgfmt -p sglsp --all-targets -- -D warnings`
- [ ] 8.8 Re-run the baseline probe matrix: every previously rejected form in
  scope now compiles, and every previously accepted form still compiles.
- [ ] 8.9 Realworld loops green for the rewritten fixtures (non-locked where a
  v1 lockfile blocks the locked gate per D8).
- [ ] 8.10 `openspec validate mainstream-usability-p0-p5 --strict`

## Archive Gate

- [ ] `openspec validate mainstream-usability-p0-p5 --strict` passes.
- [ ] `Option`/`Result` are enums; no in-tree code constructs a sentinel payload
  to express absence or failure.
- [ ] `for` iterates every collection listed in P1, with array/slice/range
  lowering unchanged.
- [ ] A successful `sgc run` prints no compiler instrumentation at default
  verbosity, and `--verbose` restores it.
- [ ] All compiler diagnostics are English with unchanged stable codes.
- [ ] Flagship examples read idiomatically and pass their loops.
- [ ] `sgfmt` honors `max_width` for every block form; idiomatic multi-line
  bodies pass `sgfmt --check`.
- [ ] Language reference and `SUPPORT_MATRIX.md` updated with proof links.
