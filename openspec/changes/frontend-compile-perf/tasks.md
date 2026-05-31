## 1. Phase 0 — Profiling + lowering O(n²) fix (landed)

- [x] 1.1 Add opt-in `SENGOO_PHASE_TIMINGS` printer at the `build`/`run` command layer (stderr, behavior-neutral) surfacing `parse/typeck/mir/mir_prune/codegen/link`.
- [x] 1.2 Sub-split the `mir` bucket into `hir_lower` / `mir_lower` / `mir_opt` in `compile_frontend_to_mir_with_phase_timings`.
- [x] 1.3 Extend `compile_phase_timings_include_expected_keys` to assert the new keys.
- [x] 1.4 Capture release baseline on a 10k-function probe; identify `mir_lower` as the #1 cost.
- [x] 1.5 Convert `LoweringContext.known_functions`/`function_sigs` to `Cow<'a, _>`; borrow on read.
- [x] 1.6 Switch the generic/lambda/async insert sites to `to_mut()` (`context_methods.rs`, `lambda_expr_helpers.rs`, `async_methods.rs`).
- [x] 1.7 Verify four suites green (compiler 575, sgc 247, runtime 46, sgpm 99) and record frontend −62% (mir_lower −99.5%).

## 2. Phase 1 — Intern type-checker storage (next)

- [ ] 2.1 Re-profile the 10k probe to confirm `typeck` is the current #1 frontend cost before editing.
- [ ] 2.2 Change `SymbolKind::{Var,Function,Type,Const,Static}` to store `InternedTyId`; keep `Symbol::get_ty` working via an owned-`Ty` materialization adapter.
- [ ] 2.3 Update the six cloner sites from the interned-types §6.1 catalog (`infer.rs:63`, `check.rs:625`, `check.rs:832`, `check/decl_helpers.rs:332`, `check/class_hierarchy_helpers.rs:33`, `check/expr_helpers.rs:32-37`) in one coordinated slice.
- [ ] 2.4 Verify four suites green after the symbol-storage slice.
- [ ] 2.5 Migrate trait/impl registry storage (`FunctionTy`/`MethodSig`/`ImplInfo` `Vec<Ty>`/`Ty`) to interned handles, materializing at lookup boundaries.
- [ ] 2.6 Verify four suites green; record before/after `typeck` phase delta on the 10k probe.

## 3. Phase 2 — Lowering overlay for materialization-heavy paths (if warranted)

- [ ] 3.1 Build a lambda/async/generic-heavy probe and measure `mir_lower` under the current `Cow::to_mut()` behavior.
- [ ] 3.2 If a regression is shown, add a per-context overlay map consulted before the shared base; make inserts O(1).
- [ ] 3.3 Verify four suites green; record the `mir_lower` delta on the heavy probe and confirm no regression on the plain 10k probe.

## 4. Phase 3+ — Data-gated backlog (re-profile before each)

- [ ] 4.1 Re-profile after Phase 1/2 to pick the next measured critical path.
- [ ] 4.2 Frontend parallelization of typeck/lowering across functions (if typeck remains dominant).
- [ ] 4.3 Finer incremental invalidation boundaries (target incremental rebuild time).
- [ ] 4.4 MIR-optimization cost control (only if `mir_opt` grows on real workloads).
- [ ] 4.5 Parser/lexer tuning (only if `parse` rises above noise).
- [ ] 4.6 Compiler peak-RSS convergence toward the C++ baseline at 100k/1000k.

## 5. Process / verification (applies to every phase)

- [ ] 5.1 Select each target from `SENGOO_PHASE_TIMINGS` data before implementing.
- [ ] 5.2 Ship each phase as a small reviewable slice with no source-language behavior change.
- [ ] 5.3 Keep the four-suite verification baseline green per slice and record a before/after measurement.
