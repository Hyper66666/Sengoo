# Sengoo v0.2 Mainstream Core Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Close M0-M4 so Sengoo has one coherent, testable native v0.2 language and toolchain contract.

**Architecture:** One umbrella coordinates five independently archivable OpenSpec changes. Existing `native-debug-info` and `http-production-serving` remain unique capability owners; v0.2 milestones consume their archive evidence instead of duplicating interfaces.

**Tech Stack:** Rust workspace, Sengoo source/runtime C bridge, LLVM-text production backend, `sgc`/`sgpm`/`sgfmt`/`sglsp`, OpenSpec, GitHub Actions.

---

### Task 1: Reconcile the v0.2 baseline

**Files:**
- Modify: `openspec/changes/v0-2-baseline-reconciliation/INVENTORY.md`
- Modify: `examples/realworld/SUPPORT_MATRIX.md`
- Modify: `README.md`
- Modify: `README.zh-CN.md`
- Test: `.github/workflows/realworld-e2e.yml`

1. Inventory active branches, active changes, archives, and documentation claims.
2. Checkpoint valuable unmerged work without destructive history rewriting.
3. Reconcile each capability to one owner and one evidence row.
4. Run the M0 commands in `tasks.md` and record the common commit SHA.
5. Archive `v0-2-baseline-reconciliation` and commit the evidence update.

### Task 2: Close ownership and borrow precision

**Files:**
- Modify: `compiler/src/typeck/borrow.rs`
- Modify: `compiler/src/mir/lowering/drop_glue_helpers.rs`
- Modify: `compiler/src/mir/lowering/try_expr_helpers.rs`
- Modify: `compiler/src/mir/lowering/loop_expr_helpers.rs`
- Test: `compiler/src/tests/drop_flag_tests.rs`
- Test: `compiler/src/tests/owned_string_tests.rs`

1. Add failing tests for temporary borrows, nested aggregates, partial moves,
   generic wrappers, and every early-exit edge.
2. Make move-path and drop-flag state explicit per initialized owning path.
3. Reject escaping or owner-invalidating borrows with stable diagnostics.
4. Prove exact-once reverse-order Drop under native execution and leak gates.
5. Commit the ownership slice before starting trait/match work.

### Task 3: Close match, trait, array, and control-flow semantics

**Files:**
- Modify: `compiler/src/typeck/check/trait_impl_helpers.rs`
- Modify: `compiler/src/typeck/check/expr_helpers.rs`
- Modify: `compiler/src/mir/lowering/match_expr_helpers.rs`
- Modify: `compiler/src/parser/derive_expander.rs`
- Test: `compiler/src/tests/trait_tests.rs`
- Test: `compiler/src/tests/generic_typeck_tests.rs`
- Test: `compiler/src/tests/drop_flag_tests.rs`

1. Add negative tests for non-exhaustive/unreachable matches and invalid guards.
2. Finish associated-type projection and static trait function resolution.
3. Complete derive coverage for supported named struct/enum shapes.
4. Define fixed-array indexing, iteration, move, and Drop behavior.
5. Run M1 conformance and archive `v0-2-language-coherence`.

### Task 4: Deliver the single developer loop

**Files:**
- Modify: `tools/sglsp/src/`
- Modify: `tools/sgfmt/src/`
- Modify: `tools/sgc/src/commands/test.rs`
- Modify: `vscode-sengoo/`
- Modify: `docs/editor-setup.md`
- Test: `tools/sglsp/tests/`
- Test: `tools/sgfmt/tests/`

1. Integrate `enhance-sglsp-smart-completion` or record equivalent evidence.
2. Archive `native-debug-info` after debugger transcripts and perf gates pass.
3. Add formatter idempotence and syntax-authority compatibility tests.
4. Add one real protocol E2E covering completion, navigation, rename, format,
   test discovery, debug launch, and stale-document rejection.
5. Archive `v0-2-developer-loop` only after the installed toolchain path passes.

### Task 5: Complete the production standard library profile

**Files:**
- Modify: `runtime/src/`
- Modify: `tools/stdlib/http.sg`
- Modify: `tools/stdlib/io.sg`
- Modify: `tools/stdlib/string.sg`
- Modify: `docs/network-runtime.md`
- Test: `tools/sgc/src/tests.rs`
- Test: `runtime/src/`

1. Complete and archive `http-production-serving` in its existing owner lane.
2. Specify and test bounded reader/writer lifecycle and partial-I/O semantics.
3. Adapt file/process/network APIs without breaking current Buffer contracts.
4. Add the pinned Unicode v0.2 baseline and invalid UTF-8 tests.
5. Run real HTTP/TLS and stream fixtures, then archive
   `v0-2-production-stdlib`.

### Task 6: Enforce the v0.2 stability contract

**Files:**
- Modify: `docs/compatibility-policy.md`
- Modify: `docs/language-reference.md`
- Modify: `tools/sgc/src/`
- Modify: `tools/sgpm/src/`
- Modify: `.github/workflows/compatibility.yml`
- Test: `examples/compat/`
- Test: `fuzz/`

1. Retain v0.1.0-rc.1 and v0.2 release-candidate compatibility fixtures.
2. Enforce edition, manifest, lockfile, diagnostic, MIR, runtime, and portable
   ABI version diagnostics.
3. Add deprecation-window tests and migration guidance.
4. Make unclassified parser/compiler/package-manager panics release blockers.
5. Pass two consecutive release-candidate matrices and archive
   `v0-2-stability-contract`.

### Task 7: Archive the umbrella

**Files:**
- Modify: `openspec/changes/sengoo-v0-2-mainstream-core/tasks.md`
- Modify: `examples/realworld/SUPPORT_MATRIX.md`
- Modify: `README.md`
- Modify: `README.zh-CN.md`

1. Verify all M0-M4 children and retained owner changes are archived.
2. Run formatting, lint, workspace tests, installed release loops, performance,
   safety, compatibility, and strict OpenSpec gates on one commit SHA.
3. Reconcile the language reference and support matrix with that SHA.
4. Archive `sengoo-v0-2-mainstream-core`.
