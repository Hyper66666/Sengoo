# Frontend Rollback Baseline

Frozen baseline profile for `frontend-1000k-perf-gate`:

- `bench/frontend-memory-baseline.json`

Source evidence:

- `README.md` advanced pipeline tables (February 2026 reference host)
- `openspec/changes/frontend-1000k-perf-gate/INVENTORY.md`

Frozen on:

- 2026-06-07

## Pinned Metrics (Sengoo)

| Bucket | Frontend compile (`compile_frontend_llvm_avg_ms`) | Frontend share (`frontend_share_pct`) | Peak RSS (`peak_rss_mb_avg`) | RSS vs C++ |
|---|---:|---:|---:|---:|
| `100k` | `153.87 ms` | `36.85%` | `140.18 MB` | — |
| `1000k` | `1589.02 ms` | `86.93%` | `1367.99 MB` | `3.14x` |

## Regression thresholds

Gate thresholds (default in `bench/scripts/advanced-kpi-gate.py`):

| Metric | 100k | 1000k |
|---|---:|---:|
| Frontend time regression vs baseline | `+10%` | `+10%` |
| E2e compile regression vs baseline | `+10%` | `+10%` |
| Frontend RSS regression vs baseline | `+10%` | `+10%` |
| Frontend share regression vs baseline | — | `+5pp` |

## Absolute targets

| Workload | Peak RSS vs C++ | Frontend share | CI |
|---|---:|---:|---|
| 100k ladder | ≤ 1.5× | ≤ 70% | hard gate |
| 1000k | ≤ 1.8× | ≤ 65% | informational until met |
| 2500k stretch | ≤ 2.0× | ≤ 70% | report-only |

CI runs 100k ladder + regression gates in hard mode (`-SkipAbsoluteTargets` skips only 1000k absolutes).

## Rollback Procedure

1. Run advanced gate and emit decision evidence:
   - `python bench/scripts/advanced-kpi-gate.py --mode hard --sample <advanced-report>.json`
2. If gate fails, apply rollback mode override:
   - `python bench/scripts/frontend-memory-rollback.py --decision <advanced-gate-decision>.json`
3. Exported override:
   - `SENGOO_FRONTEND_MEMORY_MODE=legacy`
4. Block rollout until gate is green again and compare report deltas.

## Local gate wrapper

```powershell
pwsh ./scripts/frontend-1000k-perf-gate.ps1 -RunBench -Mode hard
```

```bash
./scripts/frontend-1000k-perf-gate.sh --run-bench --mode hard
```
