## Why

1000k LOC compile workloads still peak at 3.14x C++ RSS with 87% frontend time.
This child change owns the canonical performance deltas for Pillar 5.

## What Changes

- Add 1000k RSS and frontend-share absolute targets plus regression gates to
  `frontend-compile-perf`.
- Add a preservation requirement to `frontend-build-performance` without
  modifying existing cache scenarios.
- Record pinned baselines and CI perf evidence.

## Capabilities

### New Capabilities

- None.

### Modified Capabilities

- `frontend-compile-perf`: 1000k RSS/frontend-share budgets, regression gates,
  interning/memory work evidence.
- `frontend-build-performance`: preservation of runtime fingerprint behavior during
  frontend perf work.

## Impact

- `compiler/`, `tools/sgc` pipeline, `advanced_pipeline_bench.py`, CI perf workflow
- Parent umbrella: `six-pillar-gap-closure` Pillar 5
