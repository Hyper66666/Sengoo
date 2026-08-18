# Request Latency Evidence Notes (task 8.4)

Methodology: `docs/senline-dogfood-resource-methodology.md`.

All sampler percentiles below are **request-write-complete → response-frame-complete**
wall times inside the harness. They are **not** Senline admission or sandbox latency.

## Checked-in sampler percentiles

Harness: `tools/sgc/tests/senline_worker_resource.rs`.

| Host | Label | Post-warm-up samples | p50 µs | p95 µs | p99 µs | Notes |
| --- | --- | ---: | ---: | ---: | ---: | --- |
| Local Windows x64 | smoke-1k residual-fix | 768 | 59 | 110 | 150 | After ownership fixes |
| Local Windows x64 | soak-1m (task 8.3) | 999,744 | 179 | 350 | 450 | 1e6 cases; growth ~0.07 B/case |
| GHA `windows-latest` | smoke-1k run `29573240622` | 768 | 121 | 167 | 196 | CI resource smoke; PWS metric |
| GHA `ubuntu-latest` | smoke-1k run `29573240622` | 768 | 94 | 104 | 111 | CI resource smoke; RSS metric |

### Mean request time (derived bulk corpora; not pXX)

From dual-host differential CI run `29424861027` (release corpora):

| Corpus | Cases | Windows mean µs/req | Linux mean µs/req |
| --- | ---: | ---: | ---: |
| determinism | 512 | ~492 | ~453 |
| reviewed_boundary | 10,000 | ~3,570 | ~3,830 |
| seeded_eligible | 100,000 | ~2,431 | ~2,733 |

Concurrency for V1 worker evaluation remains **one in-flight request** per worker process.

## Host labels

- Local Windows development host: Windows x64 private working set sampler.
- CI: GitHub Actions `windows-latest` / `ubuntu-latest` (resource smoke on tip run `29573240622`).

## Non-claims

Do **not** cite these figures as Senline host admission, sandbox spawn, TLS, or
end-to-end product RTT.