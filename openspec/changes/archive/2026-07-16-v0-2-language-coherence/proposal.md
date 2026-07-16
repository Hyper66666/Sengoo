## Why

Sengoo already supports ownership, automatic Drop, generics, traits, associated
types, match, arrays, and structured control flow, but the authoritative
reference still marks key interactions as Subset or Experimental. These gaps
make ordinary refactoring risky: a borrow may live longer than expected,
temporary/aggregate cleanup is not uniformly proven, match guards and payload
moves lack one complete contract, and trait-associated calls remain uneven.

M1 closes the existing language before adding more syntax.

## What Changes

- Add intraprocedural last-use borrow termination and explicit escape rules.
- Complete partial-move and exact-once Drop for temporaries, nested aggregates,
  conditional initialization, generic wrappers, and structured exits.
- Specify exhaustive/unreachable match checking, guards, and payload binding
  ownership.
- Complete associated-type projection and a minimal receiver-less trait method
  call contract.
- Define fixed-array indexing, iteration, move, and Drop semantics.
- Require stable compiler/JSON/LSP diagnostics and production-native tests for
  every rejected case.

## Capabilities

### Modified Capabilities

- `memory-management`: precise borrow end, temporary/aggregate Drop, and partial
  move requirements.
- `generics-and-traits`: associated projection and receiver-less trait method
  requirements.
- `language-reference`: match, array, and structured-control-flow completion
  requirements with executable proof.

## Impact

- Compiler type checking, borrow analysis, HIR/MIR lowering, Drop glue, match
  coverage, trait resolution, derive expansion, and diagnostics.
- `sgc` and `sglsp` diagnostic parity.
- Language reference and conformance fixtures.

## Non-Goals

- User-written lifetime parameters or a Rust-compatible borrow checker.
- Exception unwinding; cleanup is required for structured exits and `?` only.
- Multi-trait objects, `Box<dyn>`, value-receiver dyn dispatch, or Cranelift dyn
  parity.
- Const generics or dynamically sized arrays.
