# Windows SDK workload observations

Current repeat: `target/agent-completion-v2/benchmark/evidence.json`, compiler
`9b1055c5a5cb7f09c016228a44f7427b7519b6a92e69cad5993004831c3aa1e5`,
latest five-package registry manifest `21b4466d61d8289f34c12076059b29e36964e2746fdca76f61617e6965694da8`.
All source/tool/registry guards pass. Cold/hot/body-edit builds measured
25.381/10.776/19.482 seconds with module reuse 0/3, 3/0 and 2/1.
Thirty SDK ready+exit trials measured p50/p95 39.475/41.633 ms; this is not
interactive CLI startup. All sixty requests, exact stream bytes and fifteen
resource baselines pass; sampled runtime RSS peaked at 18,001,920 bytes.
These host observations retain the cache, sampling and concurrency limits
described below. The following tables record the historical 7f compiler run.

Executed on DESKTOP-FUAQMFB, Windows 11 AMD64, 12 logical CPUs, 2026-11-09.
Baseline revision `ea803f1913f9d08f6393d412b0eb4b1de3055b3e` plus the source
identities recorded in `target/agent-completion/benchmark/evidence.json`.
This is a local synthetic SDK workload, not a release or competitor benchmark.

Compiler SHA256:
`7f6343885839b8dfbcd9e03509a3da73739c1ee88d87224fd01fcb1ec5fca7d7`.
Runtime C SHA256:
`eaa900898e670775f8019a8ddc363c714861db6d9887f6c04d04cb97575995d5`.
The report pins Clang, sgpm, complete runtime/compiler sources, executable,
lockfile, input source and registry identities; all before/after checks passed.
Its SHA256 is
`9e8f811c0184849c6d47c11a66fa441843ede63d3785bfdb8c5ee17a0da0987b`.

| Build phase | Elapsed seconds | Sampled process-tree peak working set | Frontend reused/rebuilt |
| --- | ---: | ---: | ---: |
| Fresh project cache | 29.162778 | 130,256,896 bytes | 0 / 3 |
| Unchanged hot build | 12.592474 | 97,267,712 bytes | 3 / 0 |
| Only business body changed | 21.525690 | 112,054,272 bytes | 2 / 1 |

The body-only phase rebuilt one symbol and reused 105 according to the actual
codegen workset. These are one observation per phase. OS and global runtime
caches were retained, and other host builds were not controlled. Sampled sums
count shared resident pages per process; they are not unique physical memory.

Thirty fresh SDK `--startup-probe` processes emitted their ready marker and
exited: p50 43.366050 ms, p95 50.299900 ms. This includes process creation and
reaping. It does not measure Sencode's interactive/TUI readiness.

Five trials exercised sixty loopback requests with a four-request barrier.
Each trial completed four paced fragmented Unicode streams, cancelled four
burst streams after 250 ms without polling, then completed four more streams
of exactly 180,224 UTF-8 bytes each after delayed consumption. All canonical
bytes and all fifteen post-wave resource baselines passed. The application was
copied away; its temporary source and registry tree were removed before these
runs, and its PATH contained only System32.

| Observation | p50 | p95 | Samples |
| --- | ---: | ---: | ---: |
| Paced first delta | 47 ms | 141 ms | 20 streams |
| Paced completion | 453 ms | 563 ms | 20 streams |
| Delayed consumer completion | 1,484 ms | 1,500 ms | 20 streams |
| Four cancellation calls | 0 ms | 15 ms | 5 waves |
| Cleanup wait after owners dropped | 0 ms | 0 ms | 15 waves |

Monotonic values use millisecond resolution; zero does not mean zero work.
Runtime sampled process working set peaked at 18,120,704 bytes. Native worker
peak was four, request-copy sampled peak 1,648 bytes, and sampled SDK decoder
pending peak 153 bytes per stream. Native queue capacity was configured to
4,096 bytes per stream. Its actual high-water remains unavailable/null; decoder
pending, configuration limits and memory peaks are separate metrics. The host
does not expose a measurement proving the exact instant a native writer blocks.

The registry manifest used here has SHA256
`61f491c04bee663d8a99aaa700899b3d7622c0b3a7c5e9c2d16c74cd145d5df4`;
its bytes are retained in
`target/agent-completion/distribution-before-maintenance-quiescence-fix/`.
The later distribution carries the maintenance-example timing repair and a
sgllm test-driver correction. Comparing sgllm archives shows only `tests/run.py`
changed; the library source is identical. This benchmark's exact original
archive bytes remain retained and pinned by its lockfile/report.

The expanded earlier preflight intentionally failed its source freeze check
while compiler/runtime edits were still running. Its observations remain in
`target/agent-benchmark/expanded-preflight/` and are not the passing evidence
above. Formatting, Python compilation and strict OpenSpec validation passed.
TLS, remote inference, paid-provider quality/cost and other platforms were not
measured by this workload.
