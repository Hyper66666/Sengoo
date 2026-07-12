## Context

This roadmap originally coordinated eleven language-maturity pillars. P0 is now
archived and the implementation has advanced unevenly across P1/P2. The
repository snapshot in `INVENTORY.md` shows two forms of drift: code is ahead of
some tasks (especially `sgpm`), while support claims are ahead of complete
cross-platform evidence in other areas.

The roadmap therefore needs milestone semantics, not another flat feature list.

## Principles

1. A clean, reproducible mainline is a product feature.
2. Default-path depth is more valuable than experimental breadth.
3. Public support claims require executable user-path evidence.
4. One owner change owns each capability and archive decision.
5. Compatibility and resource safety are tested at every phase boundary.

## Decisions

### Decision 1: Reconcile the repository before new feature implementation

`mainline-release-baseline` is the first blocking lane. It preserves the current
worktree, reconciles it with `main`, splits changes by ownership, removes only
verified generated artifacts, and establishes a green CI baseline. No later lane
may declare completion using results from an unintegrated branch.

### Decision 2: Use capability tiers for backends

Backends have one of three tiers:

- **production**: runs the core and default-library conformance gates;
- **experimental**: opt-in, rejects unsupported programs explicitly, and does
  not define language semantics;
- **deferred**: design-only until its entry criteria pass.

LLVM-text plus clang is production for the first mainstream release. Cranelift
is experimental. WASM and bytecode are deferred until the native MIR/runtime
ABI is versioned and the default-library gate is closed.

### Decision 3: Freeze a generic collection ABI before implementation

Generic collections use a type-erased runtime storage core plus monomorphized
Sengoo wrappers. The internal element descriptor includes size, alignment,
move/copy policy, drop callback, and where required hash/equality/order
callbacks. Public APIs remain typed (`Vec<T>`, `HashMap<K,V>`); raw descriptors
are not user-facing.

This design must specify:

- ownership transfer for insert/remove;
- reference invalidation after mutation or reallocation;
- panic/error behavior during growth and callback failure;
- exact once-only drop behavior for live elements;
- compatibility wrappers for existing scalar APIs.

### Decision 4: Make concurrency semantics independent of scheduler algorithm

The release contract requires `Send`/`Sync`, generic `Arc`/locks/channels,
structured task lifetime, cancellation, and reactor progress without busy
polling. A fixed worker pool is acceptable. Work stealing may be added later if
benchmarks justify it and does not alter public semantics.

### Decision 5: Put adoption before alternative deployment targets

After the default library closes, registry resolution, release archives,
install/upgrade, and macOS evidence take priority over WASM and bytecode. A
language that cannot be installed and depended on reproducibly is not made more
mainstream by adding another backend.

### Decision 6: Separate feature evidence from release evidence

Each capability records four evidence levels:

1. parser/typecheck or unit evidence;
2. native integration evidence;
3. realworld package evidence;
4. release-host evidence from supported OS/architecture jobs.

`Supported` in `examples/realworld/SUPPORT_MATRIX.md` requires the level appropriate to the public
claim. Missing host evidence is `Platform-specific`, not inferred success.

### Decision 7: Avoid overlapping umbrella changes

`language-maturity-roadmap` is the sole coordinator for this program. Existing
umbrellas may remain as historical records, but new work is assigned to the
child changes in `proposal.md`. A child archive updates this roadmap and the
inventory in the same change.

The `Upstream spec/change` column in
`examples/realworld/SUPPORT_MATRIX.md` is evidence lineage and may cite archived
historical changes; it is not the active-owner registry. `INVENTORY.md` records
the single active owner for implementation work.

## Dependency order

```text
mainline-release-baseline
  -> numeric-type-system
  -> generic-collections
  -> debugger-and-test-framework
       -> package-registry-and-distribution
       -> concurrency-safety-and-async-io
            -> production-hardening-v1
                 -> mainstream default release gate
                 -> wasm-and-bytecode-backends entry review
```

Numeric, collections, and debugger work may run in parallel after Phase 0.
Registry/distribution may overlap late Phase 1 but cannot cut a release until
the default-library conformance fixture passes. Concurrency follows the generic
storage/drop foundation. Hardening consumes all preceding lanes.

## Milestone gates

### Gate A: Integrated baseline

- clean mainline with no unexplained divergence;
- OpenSpec and support-matrix facts match implementation;
- fmt, clippy, workspace tests, realworld tests, and strict validation pass;
- generated caches are excluded or documented.

### Gate B: Mainstream default library

- arbitrary user structs work in `Vec<T>` and map values;
- key/value/element drops are counted exactly once;
- numeric behavior is target-correct on supported production targets;
- debugger steps statements and reads scalar plus composite locals;
- a package fixture uses no scalar-only collection constructor.

### Gate C: External adoption

- publish -> resolve -> locked build -> test -> run passes against the reference
  registry;
- released archives install outside the checkout on Windows, Linux, and macOS;
- checksum/signature and upgrade failure paths are tested;
- one real release tag is cut.

### Gate D: Safe concurrency

- generic shared state and channels enforce `Send`/`Sync`;
- scoped children cannot leak after normal or early exit;
- reactor tests cover timer/socket/owned-handle progress on supported hosts;
- stress tests prove cancellation and close paths do not hang or double free.

### Gate E: Production readiness

- parser/typechecker/runtime fuzz targets have retained corpora;
- sanitizer and leak gates pass on reference hosts;
- ABI/versioning policy and compatibility tests exist;
- compile/runtime/RSS budgets are enforced in CI;
- flagship and official packages pass the released-toolchain loop.

## Risks and mitigations

- **Integration conflicts:** preserve a checkpoint and merge in small ownership
  slices; never resolve by discarding unexplained local edits.
- **Generic ABI unsoundness:** require drop-count, reallocation, partial-failure,
  and use-after-mutation tests before exposing public constructors.
- **Platform overclaim:** use host-tagged evidence and explicit skips that do
  not count as success.
- **Registry security scope:** reference server is not called production-hosted;
  checksums, ownership, tokens, archive traversal, and replay behavior are
  specified before deployment.
- **Backend dilution:** alternative targets cannot consume implementation
  capacity until their entry gate passes.

## Parallel execution ownership

- Lane A: compiler numeric and target semantics.
- Lane B: collection ABI/runtime/stdlib wrappers.
- Lane C: debugger metadata and native transcript tests.
- Lane D: sgpm registry/resolver/release tooling.
- Lane E: async runtime and concurrency stdlib.
- Lane F: CI, fuzzing, sanitizers, ABI, performance, and ecosystem fixtures.

Shared files (`README.md`, `examples/realworld/SUPPORT_MATRIX.md`, roadmap tasks, release workflow)
are updated only by the integration owner after lane verification.
