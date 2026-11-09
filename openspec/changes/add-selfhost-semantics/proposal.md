# SG semantic analysis and typed intermediate representation

## Why

The SG syntax tree cannot yet check or compile SG programs. Type identity,
resolution, ownership and typed lowering must move into SG before the native
backend can consume a trustworthy program without Rust frontend assistance.

## Changes

- Introduce deterministic checked type/symbol/resolution arenas shared by CLI
  and LSP, with one diagnostic model and original-source mappings.
- Port the edition-2026 supported semantic surface: declarations, overload and
  method resolution, generics/traits, numeric rules, borrow/move/Drop, closures,
  FFI and async/concurrency boundaries.
- Lower checked programs to SG HIR and explicit typed MIR control flow with
  direct/indirect calls, aggregates, places and resource cleanup.
- Differentially execute the frozen semantic corpus against the reference;
  preserve rejection behavior except explicitly documented soundness fixes.

## Boundary

This change owns M3 semantic/HIR/MIR behavior. Syntax/expansion belongs to
`add-selfhost-syntax`; machine code, linking, SDK extraction and remaining tool
ports keep their separate owners. Type-arena tests alone do not complete M3.
