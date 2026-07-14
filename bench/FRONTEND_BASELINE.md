# Frontend Production Baseline Bootstrap

Bootstrap status: pending the next perf-smoke artifact upload. The retained
advanced-pipeline JSON in this branch is reconstructed from expired GitHub
Actions logs and must be replaced by the exact uploaded raw artifact before
this baseline is treated as final CI evidence.

Bootstrap baseline profile:

- `bench/frontend-memory-baseline.json`
- `bench/results/1784010348707-advanced-pipeline.json`

Temporary source evidence: GitHub Actions run `29309313924`, retained locally
as `bench/results/1784010348707-advanced-pipeline.json`, bootstrapped on
2026-07-14 while waiting for the next workflow run to upload the exact raw
artifact.

## Pinned Metrics

| Bucket | Frontend compile | Frontend share | Peak RSS | RSS vs C++ | Full build |
| --- | ---: | ---: | ---: | ---: | ---: |
| `100k` | `394.76 ms` | `72.25%` | `82.34 MiB` | `0.70x` | `546.38 ms` |
| `1000k` | `1347.34 ms` | `90.21%` | `613.84 MiB` | `1.42x` | `1493.49 ms` |

## Blocking Budgets

Shared GitHub-hosted runners showed roughly 20% frontend variance across
successive unchanged revisions. The blocking gate therefore combines a 30%
baseline regression allowance with independent safety ceilings:

| Metric | 100k | 1000k |
| --- | ---: | ---: |
| Frontend time regression | `+30%` | `+30%` |
| Full-build regression | `+30%` | `+30%` |
| Peak-RSS regression | `+30%` | `+30%` |
| Frontend-share regression | - | `+10pp` |
| Frontend absolute ceiling | `750 ms` | `2500 ms` |
| Full-build absolute ceiling | `2000 ms` | `7000 ms` |
| Peak-RSS absolute ceiling | `300 MiB` | `1800 MiB` |

The real incremental ceiling remains `200 ms`. The 100k frontend ceiling also
applies to the all-reachable profile so a reachability regression cannot hide
behind a passing generated scale sample.

## Trend-Only Comparisons

C++ RSS ratios, frontend-share targets, and the 2.5M ladder are retained as
diagnostic trends. They are not cross-language marketing claims and do not
decide whether a Sengoo release passes. The blocking gate runs with
`--skip-absolute-targets`; this skips those comparison targets, not Sengoo's
wall-time, RSS, artifact, startup, CLI, or runtime ceilings.

## Budget Changes

A baseline or threshold change must include the previous and candidate raw
JSON reports, identify the Actions run and host profile, explain whether the
change is runner variance or a product regression, and update
`bench/PRODUCTION_BUDGETS.md` in the same review. Silent snapshot replacement
is not allowed.

## Local Gate

```powershell
pwsh ./scripts/frontend-1000k-perf-gate.ps1 -RunBench -Mode hard -SkipAbsoluteTargets
```

```bash
./scripts/frontend-1000k-perf-gate.sh --run-bench --mode hard --skip-absolute-targets
```
