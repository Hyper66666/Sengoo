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

## Still required to close 8.4

1. Post-warm-up p50/p95/p99 from a harness that timestamps each framed
   request/response pair.
2. Recorded host labels (GHA image / local SKU) attached to the percentile table.
3. Explicit non-claim statement retained in the published summary.
