# Development file-readiness provider lifecycle

The named gate is `selfhost/verify_native_readiness_provider.py`, using
`selfhost/tests/native_readiness_provider.rs` and
`selfhost/tests/native_readiness_provider_probe.c`. Its successful report is
`target/selfhost/native-readiness-provider/qualified04/evidence.json`, SHA-256
`77c51f9c587dc8b72959c4b8b6e9cfa286d115a0500e2fffd3bc83dc6a76ab9e`.

The run began and ended at revision
`3a20c17026cdbd099dd13a524e491b3c43cbadb7` on
`Windows-11-10.0.26300-SP0`, AMD64. Actual modified/untracked input bytes are
identified by the report's source fingerprints; the revision alone does not
represent those changes. All six named commands succeeded. There are 255 guarded
input paths, 71 checkout source origins, 75 artifacts, nine tool/system binaries,
sixteen Cargo configuration candidates and a 120-file C dependency closure.
Every final source/origin/artifact/tool/config change list is empty.

## Admission and actual provider binding

The caller explicitly supplies the original successful SG admission report
`target/selfhost/native-sdk/final-checkpoint/evidence.json` with SHA-256
`c9ddf42b4ceacbc838e6b291c5f3348ef0d893f10cb27a8a1a53fae00fd9b8ee`.
The verifier checks its success, unchanged-input results, all fifty-one completed
cases and retained executable hash. The catalog, prelude and every catalog source
entry must have the same bytes recorded by that admission run. The catalog hash
remains `f044410918f66b38c6059b5e4b29f94aa35ea08c748fcdd3d795a7ac4c055bbb`.
The prior report supplies declaration admission, not lifecycle evidence.

All runtime sources are captured unchanged. A separate Cargo workspace changes
only the root member list to runtime plus the test harness; its derived lock may
remove unused packages and add that harness, but every retained dependency
version and checksum must occur in the original lock. Cargo then builds offline
and locked with the actual qualified Rust compiler. Input helpers, bootstrap
lock, Cargo configurations, compiler binaries and C header closures are guarded.

Cargo JSON artifacts and actual compiler commands correlate the fresh provider
archive with the consumer's `--extern` argument. The actual `.rlib` hash is
`c7aa76c83896c7b5fe60c9b0efb47d60268c4c25984b1ac72ca99d92f00495b9`.
`llvm-nm` verifies unique defined text symbols for the five readiness operations
and file open/read/close. Rust passes typed pointers to those exports into C;
the C test independently declares the physical signatures. Both languages assert
outcome size sixteen, alignment eight and offsets zero/one/eight, and C guards
the explicit result output with canaries. Missing drop, integer result-output
and non-consuming result mutations are rejected by the correlation oracle.

## Executed resource and lifecycle cases

The executable runs with an empty PATH and a sixty-second process bound. Sixteen
cases each have one initialization scope and one hundred measured scopes. The
complete ordered case IDs zero through fifteen are checked individually, followed
by the success marker; stderr is empty.

1. Disk Ready, terminal re-poll rejection, result and five-byte read.
2. Source disk closes before polling; the retained duplicate remains valid.
3. Cancel before poll.
4. Drop before poll.
5. Actual named pipe Pending, data arrival, Ready, result and five-byte read.
6. Source pipe closes while Pending, followed by cancellation.
7. Drop Pending.
8. Empty pipe with zero timeout reports error eleven.
9. Negative timeout reports error two without a duplicate OS handle.
10. Invalid source registry handle reports error three without a duplicate handle.
11. Result consumes Pending and reports error one.
12. Null-output result still consumes Pending.
13. Cancel Ready.
14. Drop Ready.
15. Source pipe closes while Pending; later data still makes the duplicate Ready.
16. Peer pipe closure produces readiness.

Every measured start adds one state allocation; valid registration adds one OS
handle. All 1,600 scopes restore Win32 process handles and independently observed
Rust global allocation counts and bytes exactly. The global allocator delegates
to `System`; it supplies no provider cleanup behavior. Initialization permits
normal process-global registry table capacity retention and does not claim zero
cold-start heap retention. Unrelated generated-program dispatch definitions are
abort-only link traps: calling one would fail the run.

## Scope and prior trials

This qualifies the current development Rust provider's physical readiness ABI
and direct lifecycle execution. It does not qualify sleep, scheduler wakeup
propagation, typed SG Future lowering, SG runtime algorithm migration, complete
Rust registry-source/system linker-library provenance, final SDK packaging or
ordinary compiler self-building. Arbitrary or previously consumed nonzero raw
state pointers violate the unsafe ABI and are not invoked. The ordinary catalog
still has no binary artifacts, and reference-runtime providers remain forbidden
in final mode.

The earlier formal directories are retained as failed trials. qualified01
correctly rejected a fresh lock regeneration that selected a newer cached
dependency. qualified02 used unfiltered Cargo metadata and failed offline while
requesting a non-Windows dependency. The final runner filters Windows metadata
and checks retained package versions/checksums. qualified03 passed all native
operations but rejected the final output order because separate C/Rust buffers
printed the summary first; qualified04 includes an explicit C stdout flush and
passes the full ordered-output oracle. No failed trial is used as qualification.

Task and checkpoint text is excluded from the source closure to avoid a report
hash referring to itself. Proposal, design and normative spec bytes are retained.
After the checkpoint/task update, `openspec validate add-selfhost-native-sdk
--strict` and `openspec validate --all --strict` both passed; the latter checked
115 items with zero failures. Rust/C formatter checks and the scoped diff
whitespace check also passed.
