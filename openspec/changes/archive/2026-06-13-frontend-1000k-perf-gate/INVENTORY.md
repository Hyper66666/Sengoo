# Baseline Inventory (frontend-1000k-perf-gate)

Child change for `six-pillar-gap-closure` Pillar 5.

## Supersession status

**Superseded by** `compile-scale-production-gate` (June 2026). Production closure,
ladder gates (100k / 1000k / 2500k report-only), and updated canonical deltas live
in the child change. Medians and gate evidence were copied to
`openspec/changes/compile-scale-production-gate/INVENTORY.md`.

## Pinned reference host profile

| Field | Value |
| --- | --- |
| OS | Windows 10 (build 26200) |
| Shell | PowerShell |
| CPU | Reference developer host (pinned June 2026; record `wmic cpu get name` on refresh) |
| Python | 3.x (`sys.executable` from bench driver) |
| Clang++ | `clang++` on PATH (`-O2`, PCH enabled for C++ baseline) |
| Sengoo rev | `codex/mainstream-usable-loop` |
| sgc command | `sgc build main.sg -O 2 --emit-llvm -o main.ll` |
| C++ command | `clang++ -O2` with precompiled header (`bench/advanced_pipeline_bench.py`) |
| Generator | `make_scale_source_sengoo(loc)` — `fn_count = max(50, loc // 4)` |
| Pipeline mode | Default (`SENGOO_LARGE_PROJECT_MODE` enabled in non-interactive runs) |
| Sample protocol | Median of repeated runs per `SCALE_ITERS_BY_LOC` / `MEMORY_ITERS_BY_LOC` |

## Three-run medians (1000k default mode)

Source: `README.md` advanced pipeline table (February 2026 reference host, three-run medians).

| Metric | Sengoo | C++ | Ratio / share |
| --- | ---: | ---: | ---: |
| Peak RSS (MB) | 1367.99 | 435.22 | **3.14x** |
| Frontend time (ms) | 1589.02 | — | **86.93%** of e2e |
| Codegen object (ms) | 76.77 | — | 4.20% |
| Link (ms) | 162.04 | — | 8.86% |
| E2e compile (ms) | 1827.84 | 4883.70 | faster than C++ |

## Three-run medians (100k default mode)

| Metric | Sengoo | C++ |
| --- | ---: | ---: |
| Peak RSS (MB) | 140.18 | 118.50 |
| Frontend compile (ms) | 153.87 | — |
| E2e compile (ms) | 417.53 | 1074.84 |

## Absolute targets (1000k)

| Metric | Baseline | Target | Status |
| --- | ---: | ---: | --- |
| Peak RSS vs C++ | 3.14x | ≤ 1.8x | **Open** |
| Frontend share | 86.93% | ≤ 65% | **Open** |
| E2e vs C++ | 0.37x | stay faster | Met |

## Regression gates (checked-in snapshot)

Reference snapshot: `bench/frontend-memory-baseline.json`

| Metric | Threshold |
| --- | --- |
| Peak RSS vs snapshot | +10% |
| Frontend share vs snapshot | +5 percentage points |
| E2e compile vs snapshot | +10% |

## Mitigation

`sgc build --low-memory` reduces 1000k peak RSS by ~52% (672 MB measured) at the cost of weaker incremental reuse and single-thread frontend. Documented in `README.md`; not the default gate mode.

## Verification (June 2026)

| Suite | Command | Notes |
| --- | --- | --- |
| Compiler lib | `cargo test -p sengoo-compiler --lib` | Fingerprint + stream codegen parity |
| sgc | `cargo test -p sgc` | Runtime bundle fingerprint/cache tests |
| Advanced gate | `python bench/scripts/advanced-kpi-gate.py --mode hard --sample <report>.json` | Absolute + regression |
| OpenSpec | `openspec validate frontend-1000k-perf-gate --strict` | Archive gate |
