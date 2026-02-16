# Route B Performance Design (Runtime + Compile)

Date: 2026-02-15
Owner: Sengoo
Status: Approved in brainstorming

## 1. Objective
Build a 6-8 week optimization program that improves runtime and compile performance together, with benchmark-driven decisions and CI regression protection.

Target metrics:
- Runtime median improvement: >= 30%
- Full compile time reduction: >= 35%
- Incremental compile time reduction: >= 60%

## 2. Scope
In scope:
- Benchmark system in `bench/` for runtime/full/incremental compile scenarios
- `sgc bench` command family
- Profiling and machine-readable metrics output
- Runtime optimization passes and hot-path tuning
- Compile pipeline optimization (invalid rebuild removal, cache hit rate, stage parallelism)
- CI soft gate then hard gate for perf regressions

Out of scope:
- New language semantics as primary goal
- Large LSP feature expansion (deferred)
- Package ecosystem work (deferred)

## 3. Route Selection
Chosen route: Route B (layered convergence)

Why:
- Highest probability to hit both runtime and compile targets in 6-8 weeks
- Weekly measurable progress
- Lower delivery risk than all-in backend refactors

## 4. Architecture
The program is split into four layers:

### 4.1 Benchmark Layer
- Add `bench/` with three suites:
  - `bench/suites/runtime/`
  - `bench/suites/compile/`
  - `bench/suites/incremental/`
- Add baseline snapshot file: `bench/baseline.json`
- Emit per-run result files: `bench/results/<timestamp>.json`

### 4.2 Observability Layer
- Add phase-level compile timing in `sgc`:
  - parse
  - typeck
  - mir
  - codegen
  - link
- Runtime benchmark reports p50 and p95 for stable comparison
- Compile benchmark reports full and incremental times with scenario labels

### 4.3 Optimization Execution Layer
- Runtime line (weeks 2-3):
  - MIR pass ordering and pass strengthening
  - call/temp allocation path optimizations
  - data access locality in array/struct hotspots
- Compile line (weeks 4-5):
  - finer invalidation boundaries
  - better module cache hit rate
  - compile-stage parallelization where safe

### 4.4 Quality Gate Layer
- CI `perf-smoke` on each PR (small suite)
- CI `perf-nightly` daily (full suite)
- Phase switch:
  - soft gate first (warn only)
  - hard gate later (block on threshold regressions)

## 5. Data Flow
1. `sgc bench ...` executes target suite.
2. Raw timings collected per case and per stage.
3. Aggregator computes stable metrics (p50/p95, full/incremental totals).
4. Results persisted to JSON and compared against baseline.
5. CI comments or blocks depending on configured threshold mode.

## 6. Milestones
### Week 1
- Bench framework and command surface complete
- Baseline collection complete
- CI soft gate online

### Week 2-3
- Runtime hot-path optimization package delivered
- Runtime gain reaches >= 20% mid-target, then push to >= 30%

### Week 4-5
- Compile optimization package delivered
- Full compile >= 35% reduction
- Incremental compile >= 60% reduction

### Week 6-8
- Joint tuning and stabilization
- Hard gate enabled
- Final performance report

## 7. Risks and Mitigations
Risk: benchmark noise masks real changes
- Mitigation: fixed inputs, warmup discard, p50/p95 reporting

Risk: runtime gains regress compile speed (or opposite)
- Mitigation: dual KPI gate and per-PR smoke checks

Risk: broad refactor introduces instability
- Mitigation: small isolated optimization units and frequent regression runs

## 8. Acceptance Criteria
All must pass:
- Runtime suite median improvement >= 30%
- Full compile suite reduction >= 35%
- Incremental compile suite reduction >= 60%
- No CI hard-gate regression at merge time
