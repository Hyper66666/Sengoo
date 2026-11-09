# Measurement design

The owner changes only `scripts/measure_agent_workload.py`, the
`examples/bench/agent-stream` sample and this OpenSpec change. Compiler, runtime
and shared SDK sources remain frozen. Execution waits for the integration
owner's final v2 compiler and registry candidate.

## Build phases
Verify the checksummed Agent registry bundle, extract it into a private external
workspace and copy fixed sample main/business modules. Create exact version
registry dependencies and update once; lock bytes cannot change afterward.
Measure one new-project build with no project `src/build` or `target`, one
unchanged build, then change only the business function body revision 1 to 2
and rebuild. Record elapsed monotonic time, source hashes and all actual emitted
frontend reused/rebuilt and codegen workset counters. Cache-hit text is recorded
independently. Do not infer module reuse from elapsed time or missing output.

Project-cache cold does not flush OS caches or global toolchain/runtime/native
artifacts. Each build phase is one host observation, not a statistical population
or a claim of cold-machine compilation. The business change does not modify the
main source or its import/API signature. Locked dependencies come only from the
verified local registry, never checkout package overrides.

## Process memory
A Windows sampler follows only the launched PID and descendants. It reads
working-set/OS peak working-set counters on owned process handles, with target
10-ms sampling, recording actual sample gaps and process coverage. Report peak
sum of observed live process working sets separately from each process's observed
OS lifetime peak. Shared pages can be counted repeatedly in the sum; short-lived
children may be absent. This is RSS/working-set evidence, not allocator, unique
physical RAM or total-machine memory. The runtime target excludes the fixture
server/driver process. Fail if the sampler provides no usable observation.

## Four concurrent streams
The Sengoo program uses actual LlmRequestConfig/ProviderJob polling and bounded
runtime network workers. Four independent Chat Completions requests are sent to
a loopback HTTP server. A four-party barrier verifies concurrency; SSE frames
containing Chinese/emoji are split into five-byte paced fragments. Canonical
completion bytes must match the known fixture exactly. A second four-request
wave uses burst output and pauses all polling for 250 ms before explicit
cancellation. A third burst wave pauses polling for 250 ms, then polls every
5 ms and verifies 180,224 canonical UTF-8 bytes per stream. No unmeasured
native queue occupancy or exact blocking instant is inferred from this setup.

Each process trial reports per-stream first-delta/completion timings, delta
counts/bytes, native worker/handle/request-copy sampled peaks and per-stream
HTTP/SSE pending sampled high-water. Cancellation call time is distinct from
post-scope worker/request-copy cleanup waiting. After each of the three waves, String,
Buffer, bounded JsonDoc/reservation, worker, handle and request-copy counters
must return exactly to the captured baseline. The program never treats a cancel
request as evidence that a worker has already stopped.

Default five repeated trials produce transparent sample distributions and raw
records. These are synthetic plain-HTTP transport latencies, not TLS, remote-network or
model inference speed.
Runtime sources and the copied registry are deleted before execution with no
compiler/source PATH or source-runtime override. No real credential or public
network service is consulted.

## Native SDK process startup
Thirty new processes run a dedicated `--startup-probe` argument. The program
emits one validated SDK-ready JSON marker after argument parsing and exits
successfully. Measure process creation through validated output and process
reaping together. Preserve each sample and p50/p95. This measures native SDK
process startup/exit, not the Sencode TUI, provider initialization or interactive
readiness. No comparison against another language or CLI is implied.

## Queue observability boundary
Current public runtime counters expose live workers, handles and request-copy
bytes, but not native byte-queue occupancy. Record configured capacity separately
and set native queue high-water to null/unavailable. The SDK exposes pending
HTTP/SSE buffers: report the maximum per-stream sum sampled immediately before
and after poll, which may miss transients and is not a native queue measurement.
A future generic runtime metric belongs in a separate approved change.

The driver removes only its exact inventoried temporary files then empty
directories. Timeout cleanup targets only the launched managed process tree.
Every final report pins compiler, package manager, registry manifest, source,
lock and executable identities; failed observations cannot become passing data.
