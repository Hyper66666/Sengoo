# Windows self-hosting baseline and corpus

M0 freezes two different inputs: a compatibility inventory and measured daily
workloads. An inventoried test is not a passed test. Ten performance samples
are not compiler correctness, installed distribution, a clean machine or SG
self-hosting evidence.

## Corpus contract

`baseline-workloads.json` maps authoritative edition/status/support documents
to retained language, semantic, native runtime, formatter, package graph,
language-server, realworld/Agent, distribution and performance groups. The
runner expands those groups to an exact file/SHA-256 inventory and includes the
actual compiler/runtime/tool sources. The evidence records the full source
revision plus working-tree state; disk hashes bind new files not yet committed.
Changes/additions/removals during measurement invalidate the report.

The reference is `docs/language-reference.md`,
`examples/realworld/SUPPORT_MATRIX.md`, `docs/compatibility-policy.md` and the
Windows Agent preview evidence. Windows Stable and Supported-subset behavior
remains required, including existing documented rejection boundaries. Portable
WASM/bytecode, Cranelift and unsupported graphics stay separately classified.
Linux/macOS, Python versions unavailable here and installed candidate gates
remain independent; a Windows benchmark does not close them.

Keep the retained broad Rust tests/fixtures until their SG equivalents and
cross-compiler corpus drivers pass. The small performance source only defines a
repeatable daily workload; it does not narrow compatibility to that program.

## Measurement protocol

The fixed list has eight workloads: compiler startup, check, full build,
imported-body incremental build, format check, generated native runtime,
locked package metadata and embedded LSP diagnostics. Use O2 generated code.
Full build uses `--force-rebuild`; incremental edits alternate the body of one
imported function without changing its signature. Warm global runtime/native
and OS caches remain warm. No cache-flush or cold-machine claim is made.

Each workload gets one unreported warmup followed by ten measured samples.
Reference/candidate runs use separate project caches and alternate AB/BA order
by sample, keeping source, target, options and observer behavior equal. Every
incremental build is followed by an untimed exact-output execution. Record
each command, status, raw stdout/stderr, latency and memory observation.
The fixture prints the actual imported `bench_revision()` value. Incremental
verification requires exactly 7 or 8 according to that particular edit, so a
stale successful executable cannot pass merely because its result is positive.
Ten samples finish on revision 7, which is required by the native-runtime
workload. `test_baseline.py` exercises a real stale executable without a rebuild
and requires its expected-revision assertion to fail.

Root RSS is Windows `PeakWorkingSetSize`, queried through the retained process
handle after completion, so short-lived startup commands retain an OS lifetime
peak. Process-tree RSS reuses the existing Windows sampler. Its limitations
remain explicit: short-lived descendants may be missed, shared pages may be
summed twice, and external benchmark/fixture processes are excluded. The
aggregate metric uses at least the root lifetime peak. Missing OS peak data or
sampler failures fail the run; missing data is never zero-filled as a pass.

LSP uses an isolated byte-identical copy with empty PATH and no sibling `sgc`,
deliberately exercising its supported embedded analyzer. The real stdio client
initializes a workspace, opens a clean document, changes a variable to an
undefined symbol, requires the expected error, then shuts down and exits.
Record initialize, edit-to-diagnostic and complete lifecycle latencies; the
diagnostic latency is the comparison metric. External `sgc` diagnostics and
full installed LSP integration remain a separate compatibility gate. Tracing
is disabled consistently for both sides to keep stdout as framed JSON-RPC.

## Run and qualify

Pass a directory containing actual `sgc.exe`, `sgfmt.exe`, `sgpm.exe` and
`sglsp.exe`, plus the verified clang path. Tools are never guessed from PATH.
For example, after the independent release source rebuild:

```powershell
& 'C:/Users/admin/.cache/codex-runtimes/codex-primary-runtime/dependencies/python/python.exe' `
  selfhost/baseline.py `
  --reference-dir target/selfhost/seed-reference/release `
  --measurement-context quiet-host `
  --clang 'C:/Users/admin/AppData/Local/SengooToolchain/clang+llvm-19.1.7-x86_64-pc-windows-msvc/bin/clang.exe' `
  --output target/selfhost/m0
```

These are verified local development inputs, not portable installation
defaults. Native C/runtime construction still requires the current clang,
Windows SDK/MSVC and Rust toolchain. Environment values are not serialized.
The report names its tools by bytes and embedded versions rather than assuming
a directory name proves optimization or provenance.

Without matching release qualification evidence, successful measurement is
`provisional-reference-baseline`, even for files under a `release` directory.
The default `--measurement-context development` also always remains
provisional. Use `quiet-host` only after competing build/test work has finished;
this records an operator declaration, not a claim of VM isolation or absence
of ordinary background OS activity. Development/concurrent-load runs cannot
be promoted by the later qualification attachment.
Qualification JSON must contain `result: "passed"`, `profile: "release"`, a
`source_revision`, and each of the four `tools` with its measured `sha256`.
The independent qualification procedure owns its actual build/test assertions.
This runner only binds that evidence to the same measured binaries.

Use `--reference-qualification FILE` during measurement, or attach subsequently
completed evidence without rerunning or relabeling raw samples:

```text
baseline.py --qualify-report target/selfhost/m0/evidence.json --reference-qualification FILE
```

The attachment path verifies that the exact input inventory and tool bytes
remain unchanged and writes a separate checksummed `qualification.json`.
It rejects failed, changed or incomplete measurement. The original raw report
remains preserved. With `--candidate-dir` and matching qualification for both
sides, the same protocol compares medians per workload. Candidate time must
be at most 110% and root/tree peak memory at most 115%; exceeding any limit
fails. Existing compile-scale, runtime resource, artifact, installed and
compatibility gates remain required beyond this report.

## Current evidence status

The first source-release reference measurement is recorded under ignored
`target/selfhost/m0/`; its `evidence.json` is authoritative for completion,
identity and limitations. Baseline preparation does not certify seed
redistribution or a clean-machine rebuild. Seed qualification and complete
cutover remain owned by their named implementation/integration gates.

The initial M0 harness trial was collected while independent Cargo release
qualification tests were compiling on the same host. Those raw observations
are development/harness evidence with concurrent machine load, not an isolated
release performance baseline. Do not promote that trial merely because the
same binaries later pass qualification. The integration owner must run a fresh
reference measurement after the competing builds finish and explicitly retain
the new report separately from this trial.

The initial quiet-host run under `target/selfhost/m0-qualified/` also remains
superseded evidence: review found that its incremental fixture only checked a
positive return value rather than the exact changed value. It cannot certify
incremental invalidation. The strengthened fixture and explicit stale-binary
negative require a new measurement; the old raw report is preserved.

## Frozen reference

The strengthened quiet-host run is `target/selfhost/m0-frozen/evidence.json`,
SHA256 `7d25ae8661ff7d5b3117988d6ff45658f421504f51239640583c65284d73bfc5`.
It records 741 source/corpus inputs, 101 raw executions including preparation
and untimed edit verification, and ten measured samples for each workload.
Source/tool changes are empty. `reference-baseline.lock` retains its exact
inventory, tool identities, host and summaries in the source repository.

The independent source-release qualification is
`target/selfhost/reference-qualification/evidence.json`, SHA256
`0e7be2e6e305288cfe89afb93bee244a50f9122471f4cca8e0630693c0a03ee9`:
1178 compiler, 11 formatter, 64 package and 192 LSP tests passed with no skips
or filters, followed by an O2 native core/resource check. The four tools report
source revision `c70400a9cae30edc666dcccfdfc867155875853b`. No public seed asset
or clean-machine installation is implied.

| Workload | Median time |
| --- | ---: |
| Startup | 53.742 ms |
| Check | 57.480 ms |
| Full O2 build | 7382.340 ms |
| Imported-body incremental build | 7401.213 ms |
| Format check | 16.925 ms |
| Generated runtime | 10.341 ms |
| Locked package metadata | 29.527 ms |
| Embedded LSP edit-to-diagnostic | 1.593 ms |

These are this host's reference measurements and observer scope, not language
speed claims. Existing scale/resource gates and future installed reference/SDK
isolation remain required before the M8 default cutover.
