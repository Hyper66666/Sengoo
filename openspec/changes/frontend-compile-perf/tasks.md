## 1. Phase 0 — Profiling + lowering O(n²) fix (landed)

- [x] 1.1 Add opt-in `SENGOO_PHASE_TIMINGS` printer at the `build`/`run` command layer (stderr, behavior-neutral) surfacing `parse/typeck/mir/mir_prune/codegen/link`.
- [x] 1.2 Sub-split the `mir` bucket into `hir_lower` / `mir_lower` / `mir_opt` in `compile_frontend_to_mir_with_phase_timings`.
- [x] 1.3 Extend `compile_phase_timings_include_expected_keys` to assert the new keys.
- [x] 1.4 Capture release baseline on a 10k-function probe; identify `mir_lower` as the #1 cost.
- [x] 1.5 Convert `LoweringContext.known_functions`/`function_sigs` to `Cow<'a, _>`; borrow on read.
- [x] 1.6 Switch the generic/lambda/async insert sites to `to_mut()` (`context_methods.rs`, `lambda_expr_helpers.rs`, `async_methods.rs`).
- [x] 1.7 Verify four suites green (compiler 575, sgc 247, runtime 46, sgpm 99) and record frontend −62% (mir_lower −99.5%).

## 2. Phase 1 — Eliminate the typeck O(n²) (substitution accumulation)

Profiling re-ranked the Phase 1 target. After the Phase 0 lowering fix, `typeck`
dominated the frontend (96.8% at 10k). The cause was **not** symbol-storage
cloning (the original hypothesis) but an unbounded `TypeInfer.subst`: the existing
`reset_subst` was never called, so every function's fresh type-variable bindings
accumulated for the whole program while `unify` (returns `self.subst.clone()`) and
unification checkpoints cloned the full map — O(functions × total vars).

- [x] 2.1 Re-profile the 10k probe; confirm `typeck` is the #1 frontend cost and verify quadratic scaling (2.5k/5k/10k typeck = 96.6 / 422 / 9643 ms).
- [x] 2.2 Reset `TypeInfer.subst` at each function/method body entry (`check_function_decl`, `check_class_method_decl`). Behavior-preserving because type-var ids are monotonic and never reused, so cross-function subst entries can never be looked up again — clearing them only drops dead weight.
- [x] 2.3 Verify compiler suite green (575) and record result: typeck 9643 → 125 ms at 10k (**77×**), frontend 9957 → 435 ms (**22.9×**); typeck scaling now linear (23 / 55 / 125 ms at 2.5k/5k/10k).

### Data-gated follow-ups (symbol/registry interning)

The original symbol-storage migration remains valid for cutting the typeck
constant factor on signature-heavy / generic code, but profiling shows it is no
longer the critical path (primitive-typed probes barely exercise symbol clones).
Gate on a symbol-lookup-heavy probe before investing.

**Gate result (deferred):** A call/symbol-lookup-heavy probe (wide 8-arg
signatures, ~4 calls/fn ≈ 40k calls at 10k) shows typeck stays linear — 53 / 95 /
189 ms at 2.5k / 5k / 10k. The hot path is `instantiate_with_fresh_vars` +
`unify`, which must traverse and rebuild types with fresh vars; storing
`InternedTyId` would force a `materialize` (deep rebuild) before instantiation,
yielding no traversal savings while changing `get_ty`'s signature and ~6 call
sites. Deferred until a workload (deep generics / very large signatures) makes
symbol cloning a measured cost.

- [x] 2.4 Gate `SymbolKind::{Var,Function,Type,Const,Static}` storage migration on a call/symbol-heavy probe; no clone-heavy typeck regression was found, so the `InternedTyId` storage sweep is deferred.
- [x] 2.5 Leave the six interned-types §6.1 cloner sites unchanged for this slice; the measured hot path is fresh-var instantiation/unification work, not symbol-owned `Ty` clones.
- [x] 2.6 Defer trait/impl registry handle migration until a deep-generic or very-large-signature workload makes registry-owned `Ty` cloning a measured cost.

## 3. Phase 2 — Lowering overlay for materialization-heavy paths (if warranted)

- [x] 3.1 Add a reproducible lowering materialization probe generator (`bench/scripts/gen_lowering_overlay_probe.py`) and measure `Cow::to_mut()` behavior. The sync lambda/generic materialization probe (`.tmp/lowering_overlay_sync_2500.sg`) showed `mir_lower=1040.709ms`; a mixed async probe showed async lowering itself dominates, so the acceptance gate isolates the materialization path.
- [x] 3.2 Add a per-context overlay for known function names/signatures, consulted before the shared base; generic/lambda/async inserts now update the overlay instead of cloning the program-global base tables.
- [x] 3.3 Record the overlay result: sync materialization probe `mir_lower` 1040.709 → 465.712 ms (−55.2%); plain 10k probe after overlay is frontend=389.545 ms and `mir_lower=54.316ms`, so the no-materialization path does not regress. Final four-suite verification is recorded in §5.

## 4. Phase 3+ — Data-gated backlog (re-profile before each)

- [x] 4.1 Re-profile after Phase 1/2 on the plain 10k probe: parse=100.856 ms, typeck=101.097 ms, hir_lower=47.506 ms, mir_lower=54.316 ms, frontend=389.545 ms.
- [x] 4.2 Gate frontend parallelization; no single post-overlay phase dominates enough to justify a parallelization slice in this change.
- [x] 4.3 Gate finer incremental invalidation; left as future work because this change's evidence came from full frontend probes, not incremental rebuild traces.
- [x] 4.4 Gate MIR-optimization cost control; `mir_opt` remains effectively zero on the O0 frontend probes.
- [x] 4.5 Land a small parser/lexer allocation pass (token vector preallocation and macro/derive no-op `Cow` fast paths) with no syntax or AST behavior change.
- [x] 4.6 Add native compile-bench peak-RSS reporting (`peak_rss_bytes`) and keep the in-tree frontend probe generator for 100k/1000k memory runs; actual C++ convergence remains a future benchmark target.

## 5. Process / verification (applies to every phase)

- [x] 5.1 Select each implemented target from `SENGOO_PHASE_TIMINGS` data before editing: Phase 1 targeted typeck substitution growth; Phase 2 targeted materialization table cloning; Phase 3 only landed data-gated parser/RSS support.
- [x] 5.2 Keep each phase reviewable and behavior-neutral: typeck subst reset, lowering base+overlay, parser allocation fast paths, and bench RSS reporting are all internal implementation/measurement changes.
- [x] 5.3 Keep the four-suite verification baseline green and record final counts: `sengoo-compiler --lib` 576, `sgc` 251, `sengoo-runtime --lib` 46, `sgpm` 99; `cargo fmt --check` and `git diff --check` also pass.
