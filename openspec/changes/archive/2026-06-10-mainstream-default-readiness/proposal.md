## Why

Recent work moved Sengoo from "internal workflow is possible" to "small tools,
realworld fixtures, stdlib subsets, async subsets, package workflows, and basic
graphics demos are demonstrable." The next gap is different: make Sengoo a
**mainstream default choice** for larger internal projects, not merely a language
that can run curated examples.

This change turns the remaining gaps into a priority-ordered OpenSpec program.
It intentionally avoids redoing active work already owned by
`mainstream-production-readiness` and its child changes. Instead, it defines the
next archive-ready bar after those children land.

Priority order:

1. **Compile scale**: prove 1000k LOC memory/time gates before claiming large
   repo readiness.
2. **Async and runtime maturity**: close async IO, user future, cancellation, and
   default-concurrency semantics beyond the current supported subset.
3. **Ecosystem and release defaults**: make package graph, registry workflow,
   cross-compile, IDE, release, and rollback feel routine.
4. **Stdlib thickness**: add the next mainstream modules and ergonomics only with
   portability, resource limits, and realworld proof.
5. **Language surface polish**: remove phase-only restrictions and sharpen
   diagnostics without destabilizing existing source compatibility.

## What Changes

- Add a new `mainstream-default-readiness` capability that records the
  priority-ordered acceptance contract.
- Require each priority lane to either archive an existing child change or open a
  narrower child change before implementation.
- Require support matrices and realworld fixtures to distinguish supported,
  deferred, platform-specific, and accepted-risk behavior.
- Block umbrella archive on the compile-scale gate, because large-repo compile
  confidence is the highest-leverage remaining mainstream gap.

## Capabilities

### New Capabilities

- `mainstream-default-readiness`: priority-ordered requirements for turning
  Sengoo from internally usable into mainstream-default ready.

### Modified Capabilities

- None directly. Child changes SHALL own modifications to
  `frontend-compile-perf`, `frontend-build-performance`,
  `stdlib-mainstream-usability`, async/runtime capabilities,
  `tooling-mainstream-ecosystem`, package graph capabilities, and language
  surface capabilities.

## Impact

- Affected future areas: `compiler/`, `runtime/`, `tools/stdlib/`, `tools/sgc/`,
  `tools/sgpm/`, `tools/sglsp/`, `tools/sgfmt/`, packages, docs, examples, and
  CI workflows.
- This is a coordination change. It SHOULD NOT ship broad implementation in the
  umbrella itself.
- Existing active changes remain authoritative for their current deltas:
  `compile-scale-production-gate`, `async-reactor-futures`,
  `concurrent-async-runtime`, `runtime-hardening-ffi-async`,
  `stdlib-https-tls`,
  `ecosystem-toolchain-maturity`, `stdlib-production-surface`,
  `language-surface-expansion`, and `sgpm-alias-multiversion`.

## Non-Goals

- No public package registry launch as part of this umbrella.
- No breaking source-language cleanup unless a later change explicitly proposes
  migration and compatibility policy.
- No claim of full parity with Rust, Go, Python, or TypeScript ecosystems.
- No ad hoc implementation outside OpenSpec-owned child changes.
- No archive based solely on smoke examples; production gates need benchmark,
  test, matrix, and docs evidence.
