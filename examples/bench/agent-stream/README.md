# Agent SDK workload baseline

This real `sgllm` application starts four independent ProviderJobs, consumes
five-byte paced SSE fragments and verifies canonical Chinese/emoji output. It
then starts four burst streams and cancels them after a 250 ms no-poll window,
and completes four more burst streams with deliberately delayed consumption.
Each completed burst stream contains exactly 180,224 canonical UTF-8 bytes.
Requests use a loopback synthetic peer;
there is no API credential, paid inference or competitor comparison. The local
plain-HTTP fixture does not measure TLS, remote network delay or model compute.

`scripts/measure_agent_workload.py` builds this exact source from a checksummed
external offline registry and records:

- A fresh project without its frontend/build cache, an unchanged hot build,
  and a rebuild changing only `business.sg` from revision 1 to 2.
- Elapsed time, sampled process-tree resident working set and observed OS peak
  working set for each tracked process, plus actual compiler frontend/workset
  counters when emitted. An absent counter remains absent, not invented reuse.
- Runtime first-delta/completion and cancel/cleanup distributions over repeated
  four-stream trials. Paced and delayed-consumer latency samples stay separate.
  No latency target is asserted from a handful of samples.
- Thirty fresh native SDK processes run `--startup-probe`, validating a ready
  marker after argument parsing and clean exit. Timings include creation and
  reaping; this is not Sencode interactive/TUI readiness or network startup.
- Sampled native worker/handle/request-copy counters, per-stream HTTP/SSE pending
  bytes, configured queue capacity and exact post-scope resource restoration.

Cold means **project-cache cold**, not a cold OS or machine: OS filesystem caches,
compiler/runtime artifacts and global native caches are not flushed. Working-set
sampling targets 10 ms; short-lived children can be missed. Summing process
working sets counts shared resident pages per process and is not unique physical
RAM. Runtime memory excludes the Python fixture server and measurement driver.

Native worker queue occupancy has no existing public getter. Its true high-water
field is therefore JSON null, alongside the measured SDK decoder-pending bytes
and the separate 4096-byte-per-stream capacity configuration. Decoder watermarks
are sampled before/after poll and may miss transient maxima. They are the maximum
per-stream sum of HTTP pending bytes, SSE pending bytes and assembled SSE data,
not a native transport queue or aggregate memory peak. The burst waves use a
250 ms no-poll window and 4096-byte native queues; the completion wave then polls
every 5 ms. These exercise delayed consumers and cancellation during incoming
output, but do not directly measure queue occupancy or the instant a writer
blocks. Output content, resource restoration and process working set are actual
observations. The model answer itself grows to its declared bounded size.

The executable is copied away and all copied source/registry files are removed
before runtime. The runtime PATH is System32 only; source-runtime overrides are
removed. Only test-owned processes and a private exact temporary inventory are
cleaned up. The driver requires Windows for RSS measurement and the current
matching source-development toolchain. It introduces no new runtime metric API.

Run only after the integration owner supplies a frozen compiler and matching
registry candidate:

```text
python scripts/measure_agent_workload.py --distribution path/to/distribution --sgpm path/to/sgpm.exe --sgc path/to/sgc.exe --clang path/to/clang.exe --evidence target/agent-benchmark/evidence.json --trials 5 --startup-trials 30
```

`Sengoo.toml` is a checkout convenience. The driver creates an external manifest
using exact registry versions and captures the unchanged locked dependency graph.
Current measured results belong in this directory only after the driver passes.
The completed Windows observation and exact report identity are recorded in
[EVIDENCE.md](EVIDENCE.md).
