# Route B Performance Optimization Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Deliver benchmark-driven runtime and compile performance improvements that hit Route B targets (runtime >=30%, full compile >=35%, incremental compile >=60%).

**Architecture:** Add a benchmark subsystem to `sgc` (`bench run/compile/incremental`), persist machine-readable result snapshots, and enforce regression thresholds in CI. Optimize in two focused waves: runtime hot-path/MIR pipeline first, compile invalidation/cache/parallelism second, with dual KPI gates.

**Tech Stack:** Rust workspace (`tools/sgc`, `compiler`), serde JSON output, Cargo test harness, GitHub Actions.

---

### Task 1: Create Benchmark Scaffolding and Baseline Model

**Files:**
- Create: `bench/README.md`
- Create: `bench/baseline.json`
- Create: `bench/suites/runtime/basic_loop.sg`
- Create: `bench/suites/compile/mod_tree_root.sg`
- Create: `bench/suites/incremental/change_impl_root.sg`
- Modify: `README.md`

**Step 1: Write the failing test**
```rust
// tools/sgc/tests/bench_layout_tests.rs
#[test]
fn benchmark_scaffold_exists() {
    assert!(std::path::Path::new("bench/baseline.json").exists());
    assert!(std::path::Path::new("bench/suites/runtime/basic_loop.sg").exists());
}
```

**Step 2: Run test to verify it fails**
Run: `cargo test -p sgc bench_layout_tests -- --nocapture`
Expected: FAIL with missing benchmark files.

**Step 3: Write minimal implementation**
- Add required files/directories with minimal valid content.
- Add brief benchmark usage section in `README.md`.

**Step 4: Run test to verify it passes**
Run: `cargo test -p sgc bench_layout_tests -- --nocapture`
Expected: PASS.

**Step 5: Commit**
```bash
git add bench README.md tools/sgc/tests/bench_layout_tests.rs
git commit -m "test(bench): add benchmark scaffold and baseline model"
```

### Task 2: Add `sgc bench` CLI Surface

**Files:**
- Modify: `tools/sgc/src/main.rs`
- Create: `tools/sgc/src/bench/mod.rs`
- Create: `tools/sgc/tests/bench_cli_tests.rs`

**Step 1: Write the failing test**
```rust
#[test]
fn bench_subcommands_parse() {
    let cli = sgc::build_cli();
    assert!(cli.try_get_matches_from(["sgc", "bench", "run", "runtime"]).is_ok());
    assert!(cli.try_get_matches_from(["sgc", "bench", "compile", "compile"]).is_ok());
    assert!(cli.try_get_matches_from(["sgc", "bench", "incremental", "incremental"]).is_ok());
}
```

**Step 2: Run test to verify it fails**
Run: `cargo test -p sgc bench_cli_tests -- --nocapture`
Expected: FAIL with unknown `bench` command.

**Step 3: Write minimal implementation**
- Add `bench` command with three subcommands.
- Route execution to placeholder handlers in `tools/sgc/src/bench/mod.rs`.

**Step 4: Run test to verify it passes**
Run: `cargo test -p sgc bench_cli_tests -- --nocapture`
Expected: PASS.

**Step 5: Commit**
```bash
git add tools/sgc/src/main.rs tools/sgc/src/bench/mod.rs tools/sgc/tests/bench_cli_tests.rs
git commit -m "feat(bench): add sgc bench CLI commands"
```

### Task 3: Implement Runtime Benchmark Runner

**Files:**
- Modify: `tools/sgc/src/bench/mod.rs`
- Create: `tools/sgc/src/bench/runtime.rs`
- Create: `tools/sgc/tests/bench_runtime_tests.rs`

**Step 1: Write the failing test**
```rust
#[test]
fn runtime_bench_reports_p50_p95() {
    let report = run_runtime_suite("bench/suites/runtime").unwrap();
    assert!(report.cases.len() > 0);
    assert!(report.cases[0].p50_ms > 0.0);
    assert!(report.cases[0].p95_ms >= report.cases[0].p50_ms);
}
```

**Step 2: Run test to verify it fails**
Run: `cargo test -p sgc bench_runtime_tests -- --nocapture`
Expected: FAIL with unimplemented runtime runner.

**Step 3: Write minimal implementation**
- Add deterministic runtime runner:
  - warmup iterations
  - measured iterations
  - p50/p95 calculation
- Validate output against expected program exit status.

**Step 4: Run test to verify it passes**
Run: `cargo test -p sgc bench_runtime_tests -- --nocapture`
Expected: PASS.

**Step 5: Commit**
```bash
git add tools/sgc/src/bench/mod.rs tools/sgc/src/bench/runtime.rs tools/sgc/tests/bench_runtime_tests.rs
git commit -m "feat(bench): implement runtime suite runner with p50/p95"
```

### Task 4: Implement Full Compile Benchmark with Stage Timings

**Files:**
- Modify: `tools/sgc/src/main.rs`
- Create: `tools/sgc/src/bench/compile.rs`
- Create: `tools/sgc/tests/bench_compile_tests.rs`

**Step 1: Write the failing test**
```rust
#[test]
fn compile_bench_reports_phase_timings() {
    let report = run_compile_suite("bench/suites/compile").unwrap();
    let case = &report.cases[0];
    assert!(case.total_ms > 0.0);
    assert!(case.phases.contains_key("parse"));
    assert!(case.phases.contains_key("typeck"));
    assert!(case.phases.contains_key("mir"));
    assert!(case.phases.contains_key("codegen"));
    assert!(case.phases.contains_key("link"));
}
```

**Step 2: Run test to verify it fails**
Run: `cargo test -p sgc bench_compile_tests -- --nocapture`
Expected: FAIL with missing stage timing model.

**Step 3: Write minimal implementation**
- Add phase timer instrumentation in compile path.
- Return phase and total timing report for compile suite cases.

**Step 4: Run test to verify it passes**
Run: `cargo test -p sgc bench_compile_tests -- --nocapture`
Expected: PASS.

**Step 5: Commit**
```bash
git add tools/sgc/src/main.rs tools/sgc/src/bench/compile.rs tools/sgc/tests/bench_compile_tests.rs
git commit -m "feat(bench): add full compile stage timing reports"
```

### Task 5: Implement Incremental Compile Benchmark

**Files:**
- Modify: `tools/sgc/src/bench/compile.rs`
- Create: `tools/sgc/src/bench/incremental.rs`
- Create: `tools/sgc/tests/bench_incremental_tests.rs`

**Step 1: Write the failing test**
```rust
#[test]
fn incremental_bench_detects_impl_only_change() {
    let report = run_incremental_suite("bench/suites/incremental").unwrap();
    let case = &report.cases[0];
    assert!(case.before_ms > 0.0);
    assert!(case.after_ms > 0.0);
    assert!(case.cache_reused_modules >= 1);
}
```

**Step 2: Run test to verify it fails**
Run: `cargo test -p sgc bench_incremental_tests -- --nocapture`
Expected: FAIL due to missing incremental scenario executor.

**Step 3: Write minimal implementation**
- Add scenario runner that applies controlled edits and re-runs compile.
- Track reused vs rebuilt modules in output model.

**Step 4: Run test to verify it passes**
Run: `cargo test -p sgc bench_incremental_tests -- --nocapture`
Expected: PASS.

**Step 5: Commit**
```bash
git add tools/sgc/src/bench/compile.rs tools/sgc/src/bench/incremental.rs tools/sgc/tests/bench_incremental_tests.rs
git commit -m "feat(bench): add incremental compile benchmark scenarios"
```

### Task 6: Add Result Serialization and Baseline Diff

**Files:**
- Modify: `tools/sgc/src/bench/mod.rs`
- Create: `tools/sgc/src/bench/report.rs`
- Create: `tools/sgc/tests/bench_report_tests.rs`

**Step 1: Write the failing test**
```rust
#[test]
fn bench_result_json_is_emitted_and_diffed() {
    let path = write_bench_report_for_test().unwrap();
    assert!(path.exists());
    let diff = diff_against_baseline_for_test(path).unwrap();
    assert!(diff.summary.contains("runtime") || diff.summary.contains("compile"));
}
```

**Step 2: Run test to verify it fails**
Run: `cargo test -p sgc bench_report_tests -- --nocapture`
Expected: FAIL with missing serializer/diff API.

**Step 3: Write minimal implementation**
- Serialize benchmark reports to `bench/results/<timestamp>.json`.
- Implement baseline comparison against `bench/baseline.json`.

**Step 4: Run test to verify it passes**
Run: `cargo test -p sgc bench_report_tests -- --nocapture`
Expected: PASS.

**Step 5: Commit**
```bash
git add tools/sgc/src/bench/mod.rs tools/sgc/src/bench/report.rs tools/sgc/tests/bench_report_tests.rs
git commit -m "feat(bench): add JSON report output and baseline diff"
```

### Task 7: Runtime Optimization Wave (MIR + Hot Paths)

**Files:**
- Modify: `compiler/src/mir/opt.rs`
- Modify: `compiler/src/mir/lowering.rs`
- Modify: `compiler/src/codegen/mod.rs`
- Create: `compiler/src/tests/perf_runtime_regression_tests.rs`

**Step 1: Write the failing test**
```rust
#[test]
fn mir_optimization_removes_redundant_load_store_pairs() {
    let before = compile_to_mir("...hot loop sample...");
    let after = optimize_mir(before.clone());
    assert!(count_redundant_load_store(&after) < count_redundant_load_store(&before));
}
```

**Step 2: Run test to verify it fails**
Run: `cargo test -p sengoo-compiler perf_runtime_regression_tests -- --nocapture`
Expected: FAIL showing no measurable optimization effect yet.

**Step 3: Write minimal implementation**
- Reorder existing MIR passes for hot-path benefit.
- Add one minimal safe pass for redundant load/store elimination.
- Reduce temporary value churn in hot call paths.

**Step 4: Run test to verify it passes**
Run: `cargo test -p sengoo-compiler perf_runtime_regression_tests -- --nocapture`
Expected: PASS.

**Step 5: Commit**
```bash
git add compiler/src/mir/opt.rs compiler/src/mir/lowering.rs compiler/src/codegen/mod.rs compiler/src/tests/perf_runtime_regression_tests.rs
git commit -m "perf(runtime): optimize MIR pass order and hot-path temp usage"
```

### Task 8: Compile Optimization Wave (Invalidation + Cache)

**Files:**
- Modify: `tools/sgc/src/main.rs`
- Modify: `tools/sgc/src/bench/incremental.rs`
- Create: `tools/sgc/tests/cache_invalidation_granularity_tests.rs`

**Step 1: Write the failing test**
```rust
#[test]
fn impl_only_change_does_not_rebuild_all_modules() {
    let stats = run_incremental_change_case_for_test().unwrap();
    assert!(stats.rebuilt_modules < stats.total_modules);
}
```

**Step 2: Run test to verify it fails**
Run: `cargo test -p sgc cache_invalidation_granularity_tests -- --nocapture`
Expected: FAIL because invalidation is too coarse.

**Step 3: Write minimal implementation**
- Split invalidation keys for interface vs implementation changes.
- Expand module fingerprint metadata for finer cache reuse.
- Keep cache mismatch reasons observable.

**Step 4: Run test to verify it passes**
Run: `cargo test -p sgc cache_invalidation_granularity_tests -- --nocapture`
Expected: PASS.

**Step 5: Commit**
```bash
git add tools/sgc/src/main.rs tools/sgc/src/bench/incremental.rs tools/sgc/tests/cache_invalidation_granularity_tests.rs
git commit -m "perf(compile): reduce invalid rebuilds with finer invalidation keys"
```

### Task 9: Add CI Perf Gates (Soft -> Hard)

**Files:**
- Create: `.github/workflows/perf-smoke.yml`
- Create: `.github/workflows/perf-nightly.yml`
- Create: `scripts/perf-gate.ps1`
- Create: `scripts/perf-gate.sh`
- Modify: `README.md`

**Step 1: Write the failing test**
```text
Manual CI acceptance test:
- Open PR with synthetic perf regression value in sample JSON
- Verify workflow comments warning in soft mode
```

**Step 2: Run to verify it fails**
Run locally: `pwsh ./scripts/perf-gate.ps1 --mode soft --sample bench/sample-regression.json`
Expected: Non-zero / missing script before implementation.

**Step 3: Write minimal implementation**
- Add smoke workflow for PR subset benchmarks.
- Add nightly full benchmark workflow.
- Add gate scripts supporting `soft` and `hard` threshold modes.

**Step 4: Run to verify it passes**
Run locally: `pwsh ./scripts/perf-gate.ps1 --mode soft --sample bench/sample-ok.json`
Expected: success with readable summary.

**Step 5: Commit**
```bash
git add .github/workflows/perf-smoke.yml .github/workflows/perf-nightly.yml scripts/perf-gate.ps1 scripts/perf-gate.sh README.md
git commit -m "ci(perf): add smoke/nightly perf gates with soft-hard modes"
```

### Task 10: Final Verification and Performance Report

**Files:**
- Create: `docs/plans/2026-02-15-route-b-performance-report.md`
- Modify: `bench/baseline.json`

**Step 1: Write the failing test**
```text
Acceptance checklist fails until all KPI targets are met:
- Runtime >=30%
- Full compile >=35%
- Incremental >=60%
```

**Step 2: Run verification to capture current state**
Run:
- `cargo test -p sgc -- --nocapture`
- `cargo test -p sengoo-compiler -- --nocapture`
- `sgc bench run runtime`
- `sgc bench compile compile`
- `sgc bench incremental incremental`
Expected: produce measurable report artifacts.

**Step 3: Write minimal implementation**
- Update baseline with validated stable numbers.
- Document gains, regressions, and unresolved risks.

**Step 4: Re-run verification**
Run same command set and ensure no KPI regression.
Expected: all checks green, report complete.

**Step 5: Commit**
```bash
git add bench/baseline.json docs/plans/2026-02-15-route-b-performance-report.md
git commit -m "docs(perf): publish route-b benchmark results and final KPI status"
```

## Implementation Notes
- Use @superpowers:test-driven-development for each task.
- Use @superpowers:verification-before-completion before claiming KPI success.
- Keep each commit scoped to one task.
- Prefer minimal implementation first; iterate only when benchmark data demands it.
