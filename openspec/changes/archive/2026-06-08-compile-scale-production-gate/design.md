## Targets (pinned reference host, median of three runs)

| Workload | Peak RSS vs C++ | Frontend share |
| --- | --- | --- |
| 100k | ≤1.5× (regression gate active) | ≤70% |
| 1000k | ≤1.8× | ≤65% |
| 2500k | ≤2.0× (stretch; report-only until met) | ≤70% |

## Strategy buckets

1. Frontend AST/HIR/MIR interning and pruning (no semantic change).
2. Large-source LLVM IR streaming (preserve fingerprint tests).
3. Phase-budgeted optimization: only land changes with bench proof.

## Archive rule

Change stays **open** until 1000k absolute targets pass on the reference host.
