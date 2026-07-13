## Why

Sengoo has moved beyond a syntax prototype. The repository now contains a
native compiler, ownership and automatic `Drop`, monomorphized generics and
traits, first-class strings and formatting, a broad scripting stdlib, async
runtime primitives, `sgc`/`sgpm`/`sglsp`/`sgfmt`, executable language-reference
examples, and realworld package fixtures.

The remaining problem is not feature count. It is that the default adoption
path is still fragmented:

- substantial work exists only in a large, divergent local worktree rather
  than a clean, reviewable mainline;
- many public surfaces remain scalar-specialized or platform-specific;
- active OpenSpec tasks lag behind implemented `sgpm`, compiler, debugger, and
  runtime behavior;
- releases are not yet proven by real version tags and a Windows/Linux/macOS
  install matrix;
- alternative backends compete for attention before the native ABI and default
  library are stable.

Continuing to add breadth would increase maintenance cost without making a new
user more successful. This revision turns the roadmap into a default-path
readiness program with explicit milestones and evidence gates.

## Product target

The first mainstream-usable Sengoo release targets:

- local CLI and automation programs;
- internal tools and build tooling;
- Python-adjacent native hot paths;
- lightweight network services after the concurrency lane closes.

It is not a promise to match the ecosystem breadth of Rust, Go, Python, or
TypeScript in one release. The goal is a coherent, installable, debuggable,
package-managed default path that an external team can use without repository
knowledge or scalar-only compatibility APIs.

## Proposal

Deliver the remaining work through independently archivable execution lanes:

| Phase | Change | Outcome |
| --- | --- | --- |
| 0 | `mainline-release-baseline` | Clean mainline, fact/spec reconciliation, repeatable green baseline |
| 1 | `numeric-type-system` | Complete documented numeric semantics on the production backend |
| 1 | `generic-collections` | True generic owning collections and iterator pipeline |
| 1 | `debugger-and-test-framework` | Statement stepping and live scalar/composite inspection |
| 2 | `package-registry-and-distribution` | Registry e2e plus installable checksummed and provenance-attested releases on three desktop OS families |
| 3 | `concurrency-safety-and-async-io` | Generic shared-state primitives, structured tasks, and cross-platform reactor evidence |
| 4 | `production-hardening-v1` | Fuzz/sanitizer/leak/soak/ABI/performance and ecosystem release gates |
| Post-v1 | `wasm-and-bytecode-backends` | Alternative targets only after the native MIR/runtime ABI is versioned |

The already archived P0 changes (`automatic-memory-management`,
`generics-and-trait-system`, and `first-class-strings-and-formatting`) remain the
foundation. The archived language reference and flagship application remain
evidence, but both are refreshed when the default-library and concurrency gates
close.

## Direction changes

- LLVM-text plus clang is the production backend for the first mainstream
  release. Cranelift remains an explicitly experimental fast path until it can
  run the conformance suite; numeric completion does not require full backend
  duplication.
- Generic collection representation and ownership callbacks are designed
  before more scalar wrappers are added.
- Executor correctness, cancellation, backpressure, and `Send` safety are
  release requirements. Work stealing is an optimization, not a semantic gate.
- Registry/release usability lands before WASM or a bytecode VM.
- WASM and bytecode remain tracked, but implementation begins only after a
  stable native ABI checkpoint and a fresh go/no-go review.

## Impact

- Updates the existing language-maturity umbrella and six active child changes.
- Adds `mainline-release-baseline` and `production-hardening-v1` child changes.
- Adds missing designs for generic collections, numeric backend tiers, registry
  protocol ownership, and deferred backend criteria.
- Does not change source-language behavior by itself.

## Non-goals

- Self-hosting the compiler.
- Adding new syntax, GUI/game features, HTTP/2, or more stdlib breadth before
  the default-path gates close.
- Claiming a production-hosted public registry as complete merely because a
  reference server exists.
- Treating test counts or accepted-risk matrix rows as substitutes for a real
  install/build/run scenario.
