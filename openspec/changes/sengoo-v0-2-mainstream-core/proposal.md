## Why

Sengoo v0.1.0-rc.1 proves a substantial native language and toolchain: automatic
Drop, generics, owning collections, async/concurrency, installed releases,
package locking, source debugging, and realworld projects. The remaining gap is
coherence. Several language-reference rows remain Subset or Experimental, the
editor and formatter are not yet one fully proven workflow, the HTTP server is
not production-shaped, and compatibility policy needs executable release gates.

The next program therefore optimizes for a complete native default path rather
than more experimental breadth.

## What Changes

Create a five-milestone v0.2 program:

| Milestone | Required child | Scope |
| --- | --- | --- |
| M0 | `v0-2-baseline-reconciliation` | Truthful mainline, inventory, ownership, and verification baseline |
| M1 | `v0-2-language-coherence` | Borrow/Drop precision, match/traits, arrays, and control-flow semantics |
| M2 | `v0-2-developer-loop` | LSP, formatter, test, docs, and source-debug integration |
| M3 | `v0-2-production-stdlib` | Production HTTP dependency plus bounded stream and Unicode foundations |
| M4 | `v0-2-stability-contract` | Edition, compatibility, panic, ABI, and release-candidate policy |

The umbrella owns ordering, cross-milestone evidence, and the final archive
gate. Child changes own capability deltas and can archive independently.

## Capabilities

### New Capabilities

- `sengoo-v0-2-mainstream-core`: cross-milestone requirements for a coherent
  production-native v0.2 language and toolchain.

### Modified Capabilities

None directly. Each child owns its canonical capability deltas. Existing
`native-debug-info` and `http-production-serving` remain the only owners of
their public interfaces.

## Impact

- Compiler: type checking, borrow analysis, MIR Drop/control-flow lowering,
  production codegen diagnostics.
- Tooling: `sgc`, `sgpm`, `sgfmt`, `sglsp`, VS Code extension, release workflows.
- Runtime/stdlib: bounded streams, Unicode baseline, HTTP integration.
- Documentation/evidence: authoritative language reference, compatibility
  policy, support matrix, installed realworld loops.

## Non-Goals

- Production WASI, bytecode VM, or broad Cranelift parity.
- New macro/metaprogramming systems.
- HTTP/2, WebSocket, locale data, or full Unicode normalization tables.
- New GUI/game capabilities.
- Claiming 1.0 source/ABI stability in the v0.2 wave.
