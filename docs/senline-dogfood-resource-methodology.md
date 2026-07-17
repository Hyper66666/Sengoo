# Senline Domain Worker Resource & Latency Methodology

Recorded for OpenSpec tasks **8.3** (resource soak) and **8.4** (latency).
This document is methodology and evidence policy only. It does **not** claim
Senline host admission, sandbox, or production timing.

## Scope

| Item | In scope | Out of scope |
| --- | --- | --- |
| Process under test | Single `senline_domain_worker` binary over framed stdio | Multi-worker supervisors, Senline sandbox |
| Workload | Reviewed golden/boundary fixtures + fixed-seed eligible cases | Cryptographic or network I/O |
| Hosts | Recorded Windows x64 and Linux x64 reference runners | Mobile, WASM, cross-compile hosts |
| Memory metric | Windows **private bytes** (`PrivateUsage`); Linux RSS (VmRSS) | Working-set-only counters, cgroup limits, swap policy |
| Latency | Request-to-valid-response wall time inside the harness | Admission, TLS, sandbox spawn |

## Sampler (checked-in)

Primary harness: `tools/sgc/tests/senline_worker_resource.rs`  
(reviewed-boundary requests + process sampler; companion semantic corpora stay
in `senline_worker_differential.rs`).

Summary schema version **2** gates:

| Gate | Field | Default bound |
| --- | --- | --- |
| Endpoint growth | `memory.post_warmup_endpoint_growth_bytes_per_case` | &lt; 1 KiB/case |
| OLS regression | `memory.post_warmup_regression_slope_bytes_per_case` | &lt; 1 KiB/case |
| 10k window | `memory.max_10k_window_delta_bytes` | &lt; +32 MiB |
| Handle plateau | `handles.within_plateau` | warm-up max + 16 |
| Process count | `process_count` | 1 |
| Failures | `failure_count` | 0 |

Sampling rules:

1. **Warm-up**: discard the first `N` evaluations (default `N = 256`) before
   recording memory/latency series so JIT/cache effects are excluded from the
   post-warm-up stability window.
2. **Cadence**: sample process memory, handle/FD count, and cumulative elapsed
   time every `K` cases (default `K = 100`). Write the **full** sample series
   to a JSONL file:
   `{ case_index, elapsed_ms, memory_bytes, handle_count, cases_per_second_window }`.
   The summary keeps a 5-point tail for human glance only.
3. **Windows private bytes** (metric name `private_bytes`):
   - `GetProcessMemoryInfo` / `PROCESS_MEMORY_COUNTERS_EX.PrivateUsage`
     on the worker PID (not the cargo parent). This is **not** WorkingSetSize
     and is not labeled “private working set”.
4. **Linux RSS** (metric name `rss_bytes`):
   - Read `/proc/<pid>/status` field `VmRSS` (kB → bytes) for the worker PID.
5. **Handles / FDs**:
   - Windows: `GetProcessHandleCount` on the worker process.
   - Linux: count entries under `/proc/<pid>/fd`.
6. **Process count**: always exactly one worker child for single-worker soak;
   parent harness processes are not counted toward the worker bound.
7. **Growth math**:
   - Endpoint slope: (last − first) / cases (legacy comparison key).
   - **Linear regression**: ordinary least-squares slope of `memory_bytes` vs
     `case_index` after warm-up (primary stability gate).
   - Max delta over any contiguous ~10k-case sample window after warm-up.

Artifacts land under `target/senline-resource/` (gitignored) with names:
`soak-{label}-{os}-{arch}-{timestamp}.jsonl` and a summary
`soak-{label}-{os}-{arch}-{timestamp}.summary.json`.

## Task 8.3 success criteria (must all hold)

1. At least **1,000,000** framed evaluations complete in one continuous
   single-worker session **or** a documented failure that preserves the
   degradation evidence (do not re-label sharded 5.11 success as soak success).
2. Post-warm-up memory does not show unbounded growth: OLS regression slope
   of sampled private bytes / RSS over the post-warm-up window stays within
   the reviewed bound (default **&lt; 1 KiB/case**), with no single
   ~10k-case window exceeding **+32 MiB**.
3. File/handle/FD counts stay bounded (no climb past a reviewed plateau;
   default plateau = warm-up max + 16).
4. Zero crash, hang (watchdog), malformed plan, or nondeterminism relative to
   the linked Rust oracle for the corpus used.

### Known open observation (retain)

A prior **single-process 100,000-case** run hit the 3600 s watchdog near case
**44,086** while private working set and throughput degraded. That observation
remains open under task 8.3. The sharded 5.11 result
(8 × 12,500, transcript
`16aebd9ec476d602c9c0d0082ee9e25a87c520c333d6dd3afeb314f8c39ea128`) proves
semantic equivalence only, **not** resource stability.

### 2026-07 residual growth fix (Windows investigation)

After the lambda Drop fix, investigate-45k still showed ~**92 B/case** growth.
Root cause was application-level: `worker_validate_execution_mode(String)`
consumed a by-value legacy handle without Drop (function params skip auto-Drop
for `String`/`Buffer`/`JsonDoc`). Fixed by validating `&str` and reusing the
single extracted mode string.

### 2026-07-17 1M soak (Windows x64) — case count proven; methodology v2 still open

`resource_single_worker_soak_1m` completed **1,000,000 / 1,000,000** cases in
~238 s with zero failures; handles flat at 68; post-warm-up endpoint growth
**~0.066 B/case** (noise floor); latency p50/p95/p99 = 179/350/450 µs.
Evidence: `target/senline-resource/soak-soak-1m-windows-x86_64-1784280826.summary.json`
(schema v1-era summary: endpoint growth only, no OLS/JSONL/10k-window fields).

**Task 8.3 remains partial** until a 1M re-soak is recorded under sampler
schema v2 (OLS + 10k-window + handle plateau + JSONL + correct `private_bytes`
metric name). Linux RSS sampling uses the same harness (`rss_bytes` metric)
and is exercised on GHA `ubuntu-latest` via the resource smoke step.

## Task 8.4 latency methodology

After the same warm-up:

1. Measure wall-clock **request write complete → response frame fully read**
   for each sample case on the recorded host.
2. Publish payload class (reviewed golden size band vs seeded eligible),
   concurrency (**always 1** in-flight request for V1 worker), and host
   identity (OS, arch, runner label).
3. Report p50 / p95 / p99 over the post-warm-up sample set plus mean cases/s.
4. Do **not** claim Senline admission latency, sandbox spawn cost, or end-to-end
   product RTT.

## How to run (operator notes)

```powershell
# Sampler smoke (always-on CI / local; ~1k cases + p50/p95/p99)
cargo test --release --locked -p sgc --test senline_worker_resource -- --nocapture

# Single-worker investigation near historical case 44086 (~45k, soft watchdog)
cargo test --release --locked -p sgc --test senline_worker_resource resource_single_worker_investigation_50k -- --ignored --nocapture

# Full 1M soak (only after growth is fixed)
cargo test --release --locked -p sgc --test senline_worker_resource resource_single_worker_soak_1m -- --ignored --nocapture
```

External memory sampler example (Windows, attach to worker PID printed by harness):

```powershell
while ($true) {
  $p = Get-Process -Id $WorkerPid -ErrorAction SilentlyContinue
  if (-not $p) { break }
  "{0}`t{1}" -f (Get-Date -Format o), $p.PrivateMemorySize64
  Start-Sleep -Seconds 1
}
```

## Publication rules

- Check in this methodology and any durable summary tables under `docs/`.
- Keep multi-GB JSONL series out of git; retain CI artifacts or local
  checkpoints with full hashes in the support record when claiming green.
- Never mark 8.3 complete from multi-process shards alone.
