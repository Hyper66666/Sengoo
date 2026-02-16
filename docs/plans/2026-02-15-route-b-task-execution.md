# Route B Task Execution Status

Date: 2026-02-15  
Source plan: `docs/plans/2026-02-15-route-b-performance-implementation.md`

## Progress
- [x] Task 1: Create benchmark scaffolding and baseline model
- [x] Task 2: Add `sgc bench` CLI surface
- [x] Task 3: Implement runtime benchmark runner (p50/p95 output)
- [x] Task 4: Full compile benchmark with stage timings
- [x] Task 5: Incremental compile benchmark with reuse metrics
- [x] Task 6: Result serialization + baseline diff API
- [x] Task 7: Runtime optimization wave (MIR + hot paths)
- [x] Task 8: Compile optimization wave (invalidation + cache)
- [x] Task 9: CI performance gates
- [x] Task 10: Final verification and KPI report

## Current notes
- `sgc bench run runtime` now resolves suite paths robustly and emits JSON to `bench/results/`.
- `sgc bench compile compile` now emits parse/typeck/mir/codegen/link stage timings.
- `sgc bench incremental incremental` now reports `cache_reused_modules` from dependency fingerprint reuse.
- Bench commands now print baseline diff summaries from `bench/baseline.json`.
- Benchmark generated artifacts are ignored by Git via `.gitignore`.
- Added MIR redundant load/store elimination for O2/O3 and runtime regression test coverage.
- Lowering skips `x = x` no-op stores; codegen skips redundant self-writeback stores.
- Cache metadata now splits module `interface_hash` and implementation hash for finer invalidation reasoning.
- Added granularity test: impl-only module change does not mark all modules as rebuilt.
- Added perf gate scripts (`scripts/perf-gate.ps1`, `scripts/perf-gate.sh`) with soft/hard modes.
- Added CI workflows (`perf-smoke`, `perf-nightly`) and README usage for local/CI perf gate runs.
- Final verification completed (`cargo test` for `sgc` and `sengoo-compiler`, plus runtime/compile/incremental benchmarks).
- Current KPI snapshot from 2026-02-15 rerun: runtime +6.99%, full compile -24.17%, incremental +0.27% (targets not yet met).
