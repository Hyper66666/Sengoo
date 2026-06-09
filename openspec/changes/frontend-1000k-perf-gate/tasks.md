## 1. Baseline

- [x] 1.1 Record pinned host profile and three-run medians in `INVENTORY.md`.
- [x] 1.2 Check in reference snapshot for regression gate.

## 2. Implementation

- [x] 2.1 Frontend memory reductions without weakening runtime fingerprint tests.
- [x] 2.2 Add permanent CI perf gates for absolute targets and relative regression thresholds.

## 3. Verification

- [x] 3.1 `cargo test -p sengoo-compiler --lib` (657/657).
- [x] 3.2 `cargo test -p sgc` (targeted: memory-mode + gate wiring + runtime fingerprint cache-miss tests green)
- [ ] 3.3 `advanced_pipeline_bench.py` 100k + 1000k gate on reference host (not run end-to-end; gate logic verified via `bench/sample-frontend-1000k-gate-ok.json`)
- [x] 3.4 Runtime fingerprint/cache tests still pass

## Archive Gate

- [x] `openspec validate frontend-1000k-perf-gate --strict` passes.
- [x] `openspec validate --all --strict` passes.
- [x] Canonical deltas `frontend-compile-perf` and `frontend-build-performance` are complete.
- [ ] 1000k median RSS ≤ 1.8x C++ and frontend share ≤ 65% on reference host, or this change remains open.
