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
| Memory metric | Windows private working set; Linux RSS (VmRSS) | cgroup limits, swap policy |
| Latency | Request-to-valid-response wall time inside the harness | Admission, TLS, sandbox spawn |

## Sampler (checked-in)

Primary harness: `tools/sgc/tests/senline_worker_resource.rs`  
(reviewed-boundary requests + process sampler; companion semantic corpora stay
in `senline_worker_differential.rs`).

Sampling rules:

1. **Warm-up**: discard the first `N` evaluations (default `N = 256`) before
   recording memory/latency series so JIT/cache effects are excluded from the
   post-warm-up stability window.
2. **Cadence**: sample process memory and cumulative elapsed time every
   `K` cases (default `K = 100`) and after every 1,000 cases write a JSONL line:
   `{ case_index, elapsed_ms, memory_bytes, cases_per_second_window }`.
3. **Windows private working set**:
   - Prefer `GetProcessMemoryInfo` / `PROCESS_MEMORY_COUNTERS_EX.PrivateUsage`
     via a small probe, or PowerShell
     `(Get-Process -Id $pid).PrivateMemorySize64` from an external sampler
     attached to the worker PID (not the cargo parent).
4. **Linux RSS**:
   - Read `/proc/<pid>/status` field `VmRSS` (kB → bytes) for the worker PID.
5. **Handles / FDs**:
   - Windows: `HandleCount` on the worker process.
   - Linux: count entries under `/proc/<pid>/fd`.
6. **Process count**: always exactly one worker child for single-worker soak;
   parent harness processes are not counted toward the worker bound.

Artifacts land under `target/senline-resource/` (gitignored) with names:
`soak-{os}-{arch}-{timestamp}.jsonl` and a summary
`soak-{os}-{arch}-{timestamp}.summary.json`.

## Task 8.3 success criteria (must all hold)

1. At least **1,000,000** framed evaluations complete in one continuous
   single-worker session **or** a documented failure that preserves the
   degradation evidence (do not re-label sharded 5.11 success as soak success).
2. Post-warm-up memory does not show unbounded growth: linear regression slope
   of sampled private working set / RSS over the post-warm-up window stays
   within a reviewed bound (record the bound with the summary; default review
   gate is **&lt; 1 KiB/case** average growth after warm-up, with no single
   10k-case window exceeding **+32 MiB**).
3. File/handle/FD counts stay bounded (no monotonic climb past a reviewed
   plateau; default plateau = warm-up max + 16).
4. Zero crash, hang (watchdog), malformed plan, or nondeterminism relative to
   the linked Rust oracle for the corpus used.

### Known open observation (retain)

A prior **single-process 100,000-case** run hit the 3600 s watchdog near case
**44,086** while private working set and throughput degraded. That observation
remains open under task 8.3. The sharded 5.11 result
(8 × 12,500, transcript
`16aebd9ec476d602c9c0d0082ee9e25a87c520c333d6dd3afeb314f8c39ea128`) proves
semantic equivalence only, **not** resource stability.

Until a million-evaluation single-worker series (or an explicit, fixed root
cause with regression) is recorded, task 8.3 stays open.

### 2026-07 residual growth fix (Windows investigation)

After the lambda Drop fix, investigate-45k still showed ~**92 B/case** growth.
Root cause was application-level: `worker_validate_execution_mode(String)`
consumed a by-value legacy handle without Drop (function params skip auto-Drop
for `String`/`Buffer`/`JsonDoc`). Fixed by validating `&str` and reusing the
single extracted mode string. Post-fix investigate-45k: ~**3.4 B/case**,
PWS flat near 1.1 MiB, handles flat, 45k in ~9 s. Full 1M soak remains the
close gate for task 8.3.

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
