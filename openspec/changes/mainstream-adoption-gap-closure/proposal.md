## Why

The `six-pillar-gap-closure` program closed the implementation side of the
internal-production gaps: stdlib production surface, reactor-backed async
subset, package graph maturity, language surface expansion, default toolchain
UX, and 1000k compile-scale evidence are archived with proof. Its umbrella
still tracks final verification, but Sengoo is now usable for committed
internal projects while this wave closes outsider adoption gaps.

The next blocker set is different in kind: it is no longer "can an internal
team that built the compiler use it", but "can a team that did not build the
compiler adopt it". Audit of `examples/realworld/SUPPORT_MATRIX.md`, the
archived change record, and the compiler source originally showed four
structural adoption gaps; the cancellation pillar has since been implemented
and archived, while the remaining pillars stay active:

1. **No source-level debugging.** The native backend emits no LLVM debug
   metadata at all (no `!dbg` locations, no DI nodes anywhere under
   `compiler/src/`). `docs/debugging-native.md` documents attaching `lldb` or
   WinDbg to a `sgc build` artifact, but without line tables a developer
   steps through assembly, not Sengoo source. Every mainstream language
   ships breakpoints-on-source-lines as table stakes.
2. **Cancellation semantics needed a bounded production subset.** Three
   SUPPORT_MATRIX rows - task cancellation boundaries, select loser
   cancellation, and process cancellation - were originally `Deferred`.
   `async-cancellation-semantics` archived the supported subset; broader
   propagation APIs remain future work.
3. **HTTP serving is not production-shaped.** `async-http-serving` archived
   with reactor-backed dynamic serving, but explicitly deferred TLS server,
   keep-alive, streaming bodies, and callback handlers. A serial
   `Connection: close` plaintext server demonstrates the runtime; it does not
   host a real service.
4. **There is no way to obtain the toolchain except building it.** Adopters
   must clone the repository and run `cargo build --release`.
   `docs/internal-release.md` documents a release process, but no versioned
   prebuilt `sgc`/`sgpm`/`sgfmt`/`sglsp` binaries, checksums, or install
   scripts exist for Windows and Linux hosts.

Two further adoption blockers are tracked here only as ordered dependencies,
not owned scope:

- **Correctness trust**: `codegen-ir-correctness-and-gate` (real-CLI IR
  type consistency, multi-payload match parsing, non-blind conformance gate)
  is archived as `2026-06-11-codegen-ir-correctness-and-gate`. Native debug
  work may now edit the same IR path, but must keep that conformance gate
  green under both debug and non-debug builds.
- **1000k compile budget**: archived `compile-scale-production-gate` closed
  the reference-host RSS (0.11x C++) and frontend-share (31.83%) targets;
  this umbrella only re-runs the gate after `-g` work to prove no default-mode
  regression.

## Proposal

Deliver a four-pillar adoption program through one umbrella (this change)
plus one required child change per pillar, mirroring the proven
`six-pillar-gap-closure` structure. The umbrella owns the cross-pillar
contract, ordering, and final verification wave; each child owns its
capability deltas, design decisions, tasks, and archive gate.

The required child split is:

| Child change | Primary scope |
| --- | --- |
| `native-debug-info` | LLVM debug metadata emission, `-g`/debug-default policy, source-line breakpoints and stepping, frame variable visibility subset, debugger docs upgrade |
| `async-cancellation-semantics` | user task cancellation API and propagation contract, consuming select variant with loser cancellation, async process cancellation handles |
| `http-production-serving` | HTTP keep-alive connection reuse, callback/handler-based routing, streaming response bodies, TLS server subset on existing TLS stacks |
| `toolchain-distribution` | versioned prebuilt toolchain archives for Windows/Linux, checksums and smoke matrix, install scripts, `--version` coherence across tools |

Dependency ordering:

- `codegen-ir-correctness-and-gate` is already archived and remains a
  regression gate for `native-debug-info`; debug-info work must not weaken
  real-CLI core conformance.
- `compile-scale-production-gate` owns the passing 100k/1000k performance
  evidence and archived `frontend-1000k-perf-gate` as superseded baseline
  context. This program does not duplicate its targets; the final verification
  wave only re-runs the perf gate to prove debug-info emission did not regress
  default-mode compile performance.

### Pillar 1 â€?Source-level debugging (`native-debug-info`)

- Emit LLVM debug-info metadata (DI compile units, subprograms, locations)
  in the textual IR path so `clang` produces DWARF on POSIX and CodeView on
  Windows.
- Define the enablement policy: explicit `-g` flag on `sgc build`/`sgc run`,
  with O0/O1 defaulting per `design.md` decision D1.
- Scope v1 to function names, file/line locations, and breakpoints/stepping;
  local variable inspection is a documented stretch subset.
- Upgrade `docs/debugging-native.md` from assembly-level to source-level
  workflows with validated lldb and Windows debugger transcripts.

### Pillar 2 â€?Cancellation semantics (`async-cancellation-semantics`)

- Specify and implement a user-facing task cancellation contract on top of
  existing `spawn_task`/`cancel_task`/`task_status`: cooperative
  cancellation observed at await points, documented terminal states, and
  stable status mapping.
- Add a consuming select variant whose losers are canceled and dropped
  deterministically, keeping the existing non-canceling `select` unchanged.
- Add async process cancellation: a cancellation-capable wait on
  `ProcessHandle` that resolves promptly on kill/cancel with stable status.
- Move the three Deferred SUPPORT_MATRIX rows to supported subsets with
  proof rows.

### Pillar 3 â€?Production HTTP serving (`http-production-serving`)

- Add opt-in HTTP/1.1 keep-alive with bounded per-connection request counts
  and idle timeouts; default remains `Connection: close` until proven.
- Add handler-callback routing so applications register per-route handlers
  instead of hand-pulling requests.
- Add streaming response bodies with bounded chunk writes.
- Add a TLS server subset reusing the existing client TLS stacks (Schannel
  on Windows, rustls on POSIX), with the same no-fake-TLS rules as the
  client rows.

### Pillar 4 â€?Toolchain distribution (`toolchain-distribution`)

- Produce versioned, checksummed release archives containing `sgc`, `sgpm`,
  `sgfmt`, and `sglsp` for Windows x64 and Linux x64 from the existing
  release workflow.
- Add install scripts (PowerShell and POSIX sh) that fetch, verify, and
  place binaries on PATH.
- Make all four tools report one coherent version string sourced from the
  workspace version.
- Reuse the `docs/internal-release.md` smoke matrix as the release gate so
  archives are never published without a passing smoke run.

## Capabilities

### New Capabilities

- `mainstream-adoption-gap-closure`: umbrella requirements tying source-level
  debugging, cancellation semantics, production HTTP serving, and toolchain
  distribution into one auditable adoption program.

### Modified Capabilities

This umbrella change does not directly modify canonical capabilities. The
four required child changes own all `ADDED`, `MODIFIED`, and `REMOVED`
deltas for:

- native debug-info emission and debugger workflow capability
- async cancellation additions to the async runtime capabilities
- `stdlib-http-server` serving expansion
- release/distribution tooling capability

The umbrella MUST NOT be archived as a substitute for those child deltas.

## Impact

- Affected crates: `compiler/src/codegen/` (debug metadata), `runtime/src/`
  (cancellation, HTTP serving, TLS server), `tools/stdlib/` (async/process/
  http wrappers), `tools/sgc/` (`-g` flag, version plumbing), `tools/sgpm`,
  `tools/sgfmt`, `tools/sglsp` (version coherence), `.github/workflows/`
  (release packaging), `docs/`, `examples/realworld/SUPPORT_MATRIX.md`.
- Affected active changes: consumes archived `codegen-ir-correctness-and-gate`
  for IR-stability; re-runs the perf gate proven by archived
  `compile-scale-production-gate` in final verification without duplicating it.
- Existing public APIs remain source-compatible: cancellation and keep-alive
  surfaces are additive; the non-canceling `select` and `Connection: close`
  defaults are unchanged until child specs say otherwise.

## Non-Goals

- No macOS host support claim in this wave; distribution covers Windows x64
  and Linux x64 only, and macOS remains a documented evaluation item.
- No public package registry hosting or external marketing commitments.
- No full debug-adapter (DAP) implementation; v1 is native debugger
  (lldb/WinDbg) source-level support, not an IDE debug UI.
- No HTTP/2, websockets, or proxy features; keep-alive and streaming are
  HTTP/1.1 only.
- No JSON streaming, Unicode breadth, or iterator-combinator stdlib waves;
  they stay tracked in `INVENTORY.md` as future candidates.
- No preemptive threading model; cancellation remains cooperative at await
  boundaries.
