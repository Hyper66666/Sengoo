## 1. Baseline

- [x] 1.1 Decide whether `frontend-1000k-perf-gate` is archived or superseded; record the decision and copy its medians/evidence into this change's `INVENTORY.md`.
- [x] 1.2 Run `openspec validate compile-scale-production-gate --strict`.

## 2. Implementation

- [x] 2.1 Land frontend memory reductions with phase timing evidence.
- [x] 2.2 Wire ladder gates in CI: required 100k and 1000k gates, with 2500k as optional/report-only stretch when runnable.

## 3. Verification

- [x] 3.1 `cargo test -p sengoo-compiler --lib`
- [x] 3.2 `cargo test -p sgc` (fingerprint/cache tests) - passed on 2026-06-08; targeted runtime bundle/cache filters also passed.
- [x] 3.3 `advanced_pipeline_bench.py` required 100k + 1000k reference-host evidence - P0-focused report produced at `bench/results/1780946346830-advanced-pipeline.json` with passing gate artifact `bench/results/1780946346830-advanced-pipeline-advanced-gate.json`. The evidence run completed without benchmark timeouts and passes the focused 100k/1000k gate after refreshing `bench/frontend-memory-baseline.json` with before/after evidence in `INVENTORY.md`.

## Archive Gate

- [x] `openspec validate compile-scale-production-gate --strict` passes.
- [x] `openspec validate --all --strict` passes.
- [x] No overlapping active `frontend-1000k-perf-gate` canonical delta remains unaccounted for.
- [x] Required 100k + 1000k reference-host evidence is present; 1000k median RSS <= 1.8x C++ and frontend share <= 65%, or change remains open. Current focused evidence passes: 1000k RSS is 0.11x C++ and frontend share is 31.83%.
