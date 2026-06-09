## Why

Large-repo compile cost remains the top mainstream gap: 1000k peak RSS is 3.14×
C++ and frontend share is ~87%. `frontend-1000k-perf-gate` established baselines
and regression CI; this child change owns **production closure** of absolute targets.

## What Changes

- Enforce 1000k absolute RSS ≤1.8× and frontend share ≤65% on the pinned host.
- Add intermediate ladder gates (100k, 2500k) so progress is measurable before 1000k closes.
- Preserve runtime fingerprint/cache identity during optimizations.
- Decide and record whether `frontend-1000k-perf-gate` is archived or explicitly
  superseded before canonical promotion; do not keep two active changes editing
  the same 1000k canonical requirements.

## Capabilities

### New Capabilities

- None.

### Modified Capabilities

- `frontend-compile-perf`: production absolute gates, ladder workloads, archive criteria.
- `frontend-build-performance`: preservation requirement during scale work.

## Impact

- `compiler/`, `tools/sgc`, `bench/`, CI perf workflows
- Parent umbrella: `mainstream-production-readiness` Block 1

## Prerequisites

- Before editing canonical `frontend-compile-perf` or `frontend-build-performance`
  requirements, either archive `frontend-1000k-perf-gate` or add a supersession
  note there and copy its benchmark evidence into this change's `INVENTORY.md`.
