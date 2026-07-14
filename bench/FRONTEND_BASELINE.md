# Frontend Production Baseline

Retained baseline profile:

- `bench/frontend-memory-baseline.json`
- `bench/results/1784029453395-advanced-pipeline.json`

Source evidence: GitHub Actions run `29327347740`, retained locally as
`bench/results/1784029453395-advanced-pipeline.json`.

The Actions artifact is `production-performance-evidence` (artifact ID
`8309522200`, artifact digest
`sha256:f999a550126ab198f13cbb33c878f60f9d1bf4ff9787221ccb0ab96d5d9d497f`).
The retained raw report has SHA-256
`ce52f25330e860c65919bad4b3831017c629158f91a127b3686c794876a32d29`.
Its artifact filename timestamp (`1784029453395`) is the file-emission time;
the stable report ID uses the JSON's internal `generated_at_unix_ms`
(`1784029453392`). The raw JSON is retained byte-for-byte and is not amended
with workflow metadata.

## Pinned Metrics

| Bucket | Frontend compile | Frontend share | Peak RSS | RSS vs C++ | Full build |
| --- | ---: | ---: | ---: | ---: | ---: |
| `100k` | `379.46 ms` | `71.47%` | `82.94 MiB` | `0.70x` | `530.93 ms` |
| `1000k` | `1373.47 ms` | `89.76%` | `614.06 MiB` | `1.42x` | `1530.08 ms` |

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
