# Request Latency Evidence Notes (task 8.4 partial)

Methodology: `docs/senline-dogfood-resource-methodology.md`.

These numbers are **bulk harness wall times** from dual-host differential CI
run `29424861027` (release corpora), divided by case count. They are **not**
instrumented p50/p95/p99 request-to-response samples and **must not** be cited
as Senline admission or sandbox latency.

## Mean request time (derived)

| Corpus | Cases | Windows elapsed_ms | Windows mean µs/req | Linux elapsed_ms | Linux mean µs/req |
| --- | ---: | ---: | ---: | ---: | ---: |
| determinism | 512 | 252 | ~492 | 232 | ~453 |
| reviewed_boundary | 10,000 | 35,697 | ~3,570 | 38,296 | ~3,830 |
| seeded_eligible | 100,000 | 243,051 | ~2,431 | 273,286 | ~2,733 |

Notes:

- Determinism corpus includes two full passes (fresh_processes=2) in the
  differential test; the published artifact `elapsed_millis` is for the recorded
  outcome object (single pass used for the digest).
- Seeded eligible uses 8 fresh worker processes (sharded). Mean per request is
  still useful as a host reference but is not single-worker soak latency.
- Concurrency for V1 worker evaluation remains **one in-flight request** per
  worker process.

## Checked-in sampler (task 8.4 progress)

Harness: `tools/sgc/tests/senline_worker_resource.rs`  
(`resource_sampler_smoke_single_worker_with_latency_percentiles`, release).

| Host | Label | Post-warm-up samples | p50 µs | p95 µs | p99 µs | Notes |
| --- | --- | ---: | ---: | ---: | ---: | --- |
| Local Windows x64 | smoke-1k pre-fix | 768 | 415 | 595 | 671 | Pre lambda-Drop fix |
| Local Windows x64 | smoke-1k post-fix | 768 | 108 | 212 | 757 | After lambda `String` Drop glue |
| Local Windows x64 | investigate-45k pre-fix (~29k @900s) | 28,758 | 20,417 | 124,655 | 179,286 | Dominated by leak slowdown |
| Local Windows x64 | investigate-45k post-fix (45k complete) | 44,744 | 1,288 | 2,631 | 3,155 | Full window under growth bound |

Metric: request-write-complete → response-frame-complete wall time inside the
harness. **Not** Senline admission or sandbox timing.

## Still required to close 8.4

1. Dual-host (Windows + Linux) short-window p50/p95/p99 on a **stable**
   post-warm-up segment (or after the 8.3 leak is fixed).
2. Recorded host labels (GHA image / local SKU) attached to the percentile table.
3. Keep the non-claim statement in any published summary.
