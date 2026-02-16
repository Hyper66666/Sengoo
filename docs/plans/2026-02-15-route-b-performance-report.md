# Route B Performance Report

Date: 2026-02-15  
Plan: `docs/plans/2026-02-15-route-b-performance-implementation.md`

## Verification Commands

Executed:

```powershell
cargo test -p sgc -- --nocapture
cargo test -p sengoo-compiler -- --nocapture
cargo run -p sgc -- bench run runtime
cargo run -p sgc -- bench compile compile
cargo run -p sgc -- bench incremental incremental
```

All test suites passed:
- `sgc`: 14 passed
- `sengoo-compiler`: 209 passed, 1 ignored

Benchmark artifacts:
- `bench/results/1771170530661-runtime-runtime.json`
- `bench/results/1771170531497-compile-compile.json`
- `bench/results/1771170534714-incremental-incremental.json`

## KPI Summary

Targets:
- Runtime median improvement >= 30%
- Full compile reduction >= 35%
- Incremental compile reduction >= 60%

Measured (current run):

| KPI | Target | Measured | Status |
| --- | --- | --- | --- |
| Runtime median improvement | >= 30.00% | 6.99% | Not met |
| Full compile reduction (vs baseline) | >= 35.00% | -24.17% | Not met |
| Incremental compile reduction (same-run before/after) | >= 60.00% | 0.27% | Not met |

## Raw Metrics

- Runtime `basic_loop.sg`:
  - baseline p50: 157.54ms
  - current p50: 146.54ms
  - improvement: 6.99%
- Compile `mod_tree_root.sg`:
  - baseline total: 902.25ms
  - current total: 1120.36ms
  - reduction: -24.17%
  - dominant stage: `link` (1119.98ms)
- Incremental `change_impl_root.sg`:
  - current before: 1117.39ms
  - current after: 1114.42ms
  - same-run reduction: 0.27%
  - `cache_reused_modules`: 1

## Conclusion

Route B implementation tasks are complete, but KPI targets are currently unmet in this environment and run configuration.  
Baseline case values remain frozen in `bench/baseline.json` to keep comparisons consistent until the next optimization wave improves compile and incremental paths materially.
