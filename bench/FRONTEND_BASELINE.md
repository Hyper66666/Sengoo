# Frontend Rollback Baseline

Frozen baseline profile:

- `bench/frontend-memory-baseline.json`

Source report:

- `bench/results/1771352687788-advanced-pipeline.json`

Frozen on:

- 2026-02-18

## Pinned Metrics (Sengoo)

| Bucket | Frontend compile (`compile_frontend_llvm_avg_ms`) | Frontend peak RSS (`peak_rss_mb_avg`) |
|---|---:|---:|
| `100k` | `481.39 ms` | `142.02 MB` |
| `1000k` | `4325.08 ms` | `1387.87 MB` |

## Hard Rollback Thresholds

Gate thresholds (default in `bench/scripts/advanced-kpi-gate.py`):

| Metric | 100k | 1000k |
|---|---:|---:|
| Frontend regression vs baseline | `+12%` | `+12%` |
| Frontend RSS regression vs baseline | `+12%` | `+12%` |

Absolute safety ceilings kept in gate:

- `scale/100000/frontend <= 300ms`
- `scale/1000000/frontend <= 7000ms`
- `compile_memory_compare/100000/sengoo/peak_rss_mb_avg <= 300MB`
- `compile_memory_compare/1000000/sengoo/peak_rss_mb_avg <= 1800MB`

## Rollback Procedure

1. Run advanced gate and emit decision evidence:
   - `python bench/scripts/advanced-kpi-gate.py --mode hard --sample <advanced-report>.json`
2. If gate fails, apply rollback mode override:
   - `python bench/scripts/frontend-memory-rollback.py --decision <advanced-gate-decision>.json`
3. Exported override:
   - `SENGOO_FRONTEND_MEMORY_MODE=legacy`
4. Block rollout until gate is green again and compare report deltas.

## Latest Verification (2026-02-18)

Sample:

- `bench/results/1771363002202-advanced-pipeline.json`

Comparison vs frozen baseline:

| Bucket | Metric | Baseline | Latest | Delta |
|---|---|---:|---:|---:|
| `100k` | Frontend compile | `481.39 ms` | `484.97 ms` | `+0.74%` |
| `100k` | Frontend peak RSS | `142.02 MB` | `140.44 MB` | `-1.12%` |
| `1000k` | Frontend compile | `4325.08 ms` | `4179.16 ms` | `-3.37%` |
| `1000k` | Frontend peak RSS | `1387.87 MB` | `1378.75 MB` | `-0.66%` |

Rollback evidence in report:

- `rollback_evidence.gate_decision = pass`
