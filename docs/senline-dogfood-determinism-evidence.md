# Cross-host Determinism Evidence (task 5.9)

Recorded from GitHub Actions `core-conformance` run
[`29424861027`](https://github.com/Hyper66666/Sengoo/actions/runs/29424861027)
on commit `e9f1b1168` (jobs green for dual-host differential; core-language failed
on an unrelated evidence-hash drift fixed later).

## Claim

Identical inputs produce byte-equivalent normalized plans across:

1. two fresh worker processes on the same host (cross-process), and
2. Windows x64 and Linux x64 hosts (cross-host transcript digests).

## Transcript digests (must match across OS)

| Corpus | Cases | Fresh processes | Windows `transcript_sha256` | Linux `transcript_sha256` | Match |
| --- | ---: | ---: | --- | --- | --- |
| determinism | 512 | 2 | `bd6acd82479bd6219cbf8e96601313e79f01bb518cee5a98f137be3e40f9729c` | `bd6acd82479bd6219cbf8e96601313e79f01bb518cee5a98f137be3e40f9729c` | yes |
| reviewed_boundary | 10,000 | 1 | `a32f445f38e4810bc3eab9f2744ed337f48e2f5fa18521a9b05002c42126dd0b` | `a32f445f38e4810bc3eab9f2744ed337f48e2f5fa18521a9b05002c42126dd0b` | yes |
| seeded_eligible | 100,000 | 8 | `16aebd9ec476d602c9c0d0082ee9e25a87c520c333d6dd3afeb314f8c39ea128` | `16aebd9ec476d602c9c0d0082ee9e25a87c520c333d6dd3afeb314f8c39ea128` | yes |

All three corpora reported zero semantic mismatches, crashes, hangs, malformed
plans, and nondeterministic plans on both hosts.

## Test surface

- `tools/sgc/tests/senline_worker_differential.rs`
  - `identical_inputs_have_identical_raw_plan_bytes_across_fresh_processes`
  - ignored release corpora for 10k / 100k
- CI job: `Senline worker differential (${{ matrix.os }})` in
  `.github/workflows/core-conformance.yml`
- Uploaded artifacts:
  `senline-worker-differential-Windows-X64`,
  `senline-worker-differential-Linux-X64`

## Limits

- Digests prove planner/worker I/O normalization equivalence, not resource soak
  (task 8.3) or Senline pin (task 7.5 / 9.5).
- Fixture byte equality on Windows requires LF-normalized fixture reads
  (`normalize_fixture_bytes`) so CRLF checkouts do not falsify frozen hashes.
