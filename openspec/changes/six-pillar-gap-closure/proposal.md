## Why

Sengoo has passed the `mainstream-usable-loop` integration gate: committed
realworld packages, locked `sgpm` workflows, CLI/LSP diagnostics, and an
honest support matrix. The remaining gap is no longer "prove the workflow
exists"; it is "close the six structural gaps that still block confident
internal development at scale."

Those gaps are:

1. stdlib surface still MVP-shaped (Buffer/handle choreography, narrow JSON and
   collections, missing recursive IO and process pipes/background)
2. async runtime still cooperative-subset, not mainstream-grade (no IO wakeups,
   no user awaitables, bounded select, restricted future flow)
3. package graph still immature (no renamed deps, no multi-version resolution)
4. language surface still has explicit hard limits (attributes, class header
   traits, FFI arity/types, async frame restrictions)
5. compile-time memory and frontend share remain hard constraints at 1000k LOC
6. toolchain works but lacks Rust/Go/TS-grade default experience (real e2e,
   structured assertion failures, debugger, stable internal release)

This change is the umbrella OpenSpec lane that coordinates all six pillars into
one auditable closure program for internal use. It is not meant to hide six
large implementations behind one vague archive step: the umbrella owns the
cross-pillar contract, while implementation SHOULD be split into pillar-scoped
child changes that can be reviewed, reverted, and archived independently.

## Proposal

Deliver a phased, six-pillar closure program with spec-backed acceptance for
each pillar. Each pillar gets a child change with requirements, design
boundaries, tasks, verification commands, and matrix updates. Pillars may land
in parallel work streams, but this umbrella change is not done until all six
required child changes are validated and archived. Platform-specific test skips
may be documented inside a child change; an accepted-risk row cannot replace an
unimplemented pillar.

The preferred implementation split is:

| Child change | Primary scope |
| --- | --- |
| `stdlib-production-surface` | Pillar 1 stdlib String, JSON, collections, recursive IO, process/fd APIs |
| `async-reactor-futures` | Pillar 2 reactor, user futures, N-select, cancellation |
| `sgpm-alias-multiversion` | Pillar 3 renamed deps, multi-version lockfile, metadata |
| `language-surface-expansion` | Pillar 4 attributes, class trait headers, FFI widening |
| `frontend-1000k-perf-gate` | Pillar 5 1000k RSS/frontend-share gates |
| `toolchain-internal-ux` | Pillar 6 assertions, real e2e, debugger/editor/release docs |

### Pillar 1 — Stdlib production surface

- Promote owned `String` from handle type to stdlib return ABI for text-producing
  helpers where safe.
- Add string collections (`Vec<String>`, string-key maps with `String` values
  where specified).
- Expand JSON limits and ergonomics without breaking existing handle APIs.
- Add recursive directory walk/copy helpers and explicit tree-transfer status
  semantics.
- Add process stdout/stderr pipe chaining between parent/child commands and a
  background `ProcessHandle` lifecycle with wait/kill semantics.
- Add synchronous fd/terminal IO subset needed by internal CLI tools; async fd
  IO remains Pillar 2.

### Pillar 2 — Mainstream async runtime

- Introduce an IO wakeup/reactor layer for timer, socket, and file-descriptor
  readiness used by stdlib async helpers.
- Add trait-based `Future` / user-defined awaitable support with documented
  poll contract.
- Generalize `select` to 2..8 homogeneous operands with rotating poll order; losing
  branches are not canceled and are dropped through normal future cleanup.
- Relax future value-flow restrictions where sound (store, pass, return) with
  new negative tests for unsound escapes.
- Document and test task cancellation, timeout, and child-future cleanup
  boundaries.

### Pillar 3 — Package graph maturity

- Support renamed dependency keys (`[dependencies.alias]`) that resolve to
  packages whose `[package].name` differs.
- Support multiple versions of the same package name in one resolved graph with
  deterministic lockfile node ids.
- Add real `sgpm`/`sgc` end-to-end tests that compile and run realworld fixtures
  without fake tool stubs.
- Document internal registry/monorepo workflow for teams (no public registry
  launch required).

### Pillar 4 — Language surface expansion

- Expand attribute support on declarations beyond the current rejection set.
- Support class header trait lists (`class Foo: Base, TraitA, TraitB`).
- Widen FFI surface (additional arities/types) with hardening tests retained.
- Relax async frame / future restrictions that are only implementation-phase
  limits, not semantic requirements.

### Pillar 5 — Large-scale compile performance

- Set measurable targets for 1000k LOC compile peak RSS and frontend time share.
- Land frontend memory reductions beyond current `--low-memory` opt-in path.
- Add CI perf gates and regression snapshots for 100k/1000k workloads.
- Preserve correctness of incremental/native caches while improving scale.

### Pillar 6 — Default toolchain experience

- Add language/stdlib testing helpers (`assert`, `assert_eq`, structured failure
  output integrated with `sgc test`).
- Add real e2e CI for `examples/realworld` using real `sgc`/`sgpm`.
- Ship debugger workflow (minimum: documented `lldb`/native symbol path; target:
  `sglsp` debug adapter hooks if feasible in-lane).
- Publish internal-ready editor setup (`sglsp` launch config, fmt-on-save,
  diagnostic parity checklist).
- Define internal release channel (versioned binaries, smoke matrix, rollback).

## Capabilities

### New Capabilities

- `six-pillar-gap-closure`: umbrella requirements tying stdlib, async, sgpm,
  language surface, frontend performance, and toolchain experience into one
  internal-production closure program.

### Modified Capabilities

This umbrella change does not directly modify canonical capabilities. The six
required child changes own all `ADDED`, `MODIFIED`, and `REMOVED` deltas for:

- `stdlib-mainstream-usability` and `owned-string-text`
- `runtime-hardening-ffi-async` and the async language surface
- package graph maturity
- language surface expansion
- `frontend-build-performance` and `frontend-compile-perf`
- `tooling-mainstream-ecosystem`

The umbrella MUST NOT be archived as a substitute for those child deltas.

## Impact

- Affected crates: `compiler/`, `runtime/`, `tools/stdlib/`, `tools/sgc/`,
  `tools/sgpm/`, `tools/sglsp/`, `tools/sgfmt/`, `examples/realworld/`, docs,
  CI workflows.
- Affected specs: each child change updates its owned canonical capability before
  that child is archived.
- This is a multi-stream program. Expect six independently reviewable child
  changes followed by one integrating verification wave.
- Existing Buffer-output stdlib names remain source-compatible in this program.
  New owned-string helpers use additive names and require compiler/sglsp/docs
  coverage; removal of compatibility helpers requires a later breaking change.
- Lockfile multi-version support requires a versioned migration and forbids
  silent reinterpretation or rewrite of `version = 1` lockfiles.

## Non-Goals

- No public package registry product launch.
- No claim of full TLS/HTTPS parity unless platform tests prove it.
- No implicit shell execution or `sh -c` in new stdlib/process examples.
- No "complete Rust async ecosystem" parity in one lane; reactor scope is
  internal-tooling-first.
- No commercial support, licensing, or external marketing commitments.
