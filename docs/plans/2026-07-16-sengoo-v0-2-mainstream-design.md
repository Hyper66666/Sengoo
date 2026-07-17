# Sengoo v0.2 Mainstream Core Design

## Goal

Make Sengoo a coherent native general-purpose language with Go-like default
workflows, Rust-like resource safety, and first-class Python interoperability.
Community adoption is evidence for this goal, not a substitute for language
completeness.

## Product boundary

The v0.2 program strengthens the production native path. It does not expand the
experimental WASM, bytecode, Cranelift, GUI, or game surfaces. The supported
center is:

- native CLI and automation programs;
- concurrent local services;
- Python-hosted native hot paths;
- one installed `sgc`/`sgpm`/`sgfmt`/`sglsp` workflow.

## Program shape

The work is split into one umbrella and five independently archivable changes:

| Milestone | Owner change | Outcome |
| --- | --- | --- |
| M0 | `v0-2-baseline-reconciliation` | One truthful mainline, inventory, and owner map |
| M1 | `v0-2-language-coherence` | Borrow/drop, match, trait, array, and control-flow contracts close |
| M2 | `v0-2-developer-loop` | Editor, formatter, test, and debugger paths behave as one toolchain |
| M3 | `v0-2-production-stdlib` | Production HTTP plus bounded stream and Unicode foundations |
| M4 | `v0-2-stability-contract` | Edition, compatibility, panic, ABI, and release policy are executable |

`native-debug-info` remains the owner of native debug metadata.
`http-production-serving` remains the owner of HTTP handlers, keep-alive,
streaming response bodies, and TLS server support. The v0.2 children consume
those changes and own only the integration requirements around them.

## Principles

1. Complete the production native path before widening experimental backends.
2. A capability is supported only when positive, negative, and lifecycle tests
   prove the same behavior through the real CLI.
3. Stable diagnostics and documented rejection are preferable to partial silent
   behavior.
4. New public surface must reduce default-path friction or close a semantic
   hole; syntax breadth alone is not a milestone.
5. Each canonical requirement has one active implementation owner.

## Milestone gates

### M0 - Baseline reconciliation

Integrate or checkpoint all valuable branches, reconcile active and archived
OpenSpec state with `main`, and eliminate documentation claims contradicted by
tests. M0 archives only when one commit SHA passes formatting, lint, focused
compiler/runtime/tool tests, installed realworld loops, and strict OpenSpec
validation.

### M1 - Language coherence

Finish ownership precision for temporaries, nested aggregates, branches, loops,
and generic wrappers. Close match exhaustiveness and guards, associated-type
projection, static trait functions, derive shape coverage, fixed-array behavior,
and control-flow/drop interactions. Experimental trait-object extensions remain
explicitly out of scope unless their ownership model is proven on the production
backend.

### M2 - Developer loop

Land the evidence-backed `sglsp` workspace index and completion protocol, finish
the `native-debug-info` subset, prove formatter idempotence, and exercise a
single package through edit, navigate, rename, format, test, debug, and document
operations. The compiler and formatter remain syntax authorities; the language
server must not invent grammar.

### M3 - Production standard library

Archive `http-production-serving`, define a bounded stream abstraction shared by
file/process/network adapters, and provide a minimum Unicode foundation for
scalar iteration, UTF-8 validation, case-independent ASCII protocols, and
documented normalization/case-folding boundaries. Full locale support, HTTP/2,
WebSocket, and framework APIs remain follow-up work.

### M4 - Stability contract

Turn compatibility policy into tests: edition parsing, deprecation windows,
retained previous-release fixtures, manifest/lockfile/diagnostic/runtime ABI
version checks, and a no-unclassified-panic gate for public input. Two release
candidates must pass the same installed-artifact matrix before v0.2.0.

## Completion definition

The umbrella can archive when all five children and the two retained owner
changes are archived, `docs/language-reference.md` and
`examples/realworld/SUPPORT_MATRIX.md` match executable evidence, and one
recorded commit passes the full verification wave. WASI, production bytecode,
and broad Cranelift parity do not block this native v0.2 milestone.
