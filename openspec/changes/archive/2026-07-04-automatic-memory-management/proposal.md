## Why

Sengoo's default is not memory-safe. The language has a borrow checker for
references (`compiler/src/typeck/borrow.rs`), but every runtime heap resource is
released by hand:

- `examples/stdlib/20_owned_string.sg` ends with `owned.drop(); copy.drop(); buffer.free();`
- `examples/realworld/cli-json-audit/src/main.sg` ends with `values.free(); buffer.free();`
- `JsonDoc` requires `doc.close()`, process/net handles require explicit close.

This is the worst of both worlds: the rigor cost of ownership without the safety
payoff of automatic cleanup. Leaks are easy, double-free is possible, and the
code does not read like a mainstream language. The original spec
(`Sengoo_Language_Specification.md` §1.3) named RC + cycle GC, but the
implemented compiler is built around ownership, so this change completes that
design instead of bolting on a collector.

## Proposal

Adopt **move-based ownership with compiler-inserted `Drop` (RAII)** as the
single default memory model, and migrate runtime resources to it.

- Define a `Drop` trait with a single `def drop(&mut self)` method.
- Track ownership and moves in the type checker; reading a value after it has
  been moved is a compile error with a stable diagnostic code.
- Insert drop glue in MIR at the end of each owner's scope (including early
  returns, `?` propagation, `break`/`continue`, and on the panic/abort path),
  honoring move-out and conditional initialization.
- Migrate `Buffer`, `Vec<T>`, `String`, `JsonDoc`, and process/net/file handles
  to auto-drop types so `.free()/.drop()/.close()` become unnecessary in
  idiomatic code (the explicit methods remain available and source-compatible).
- Provide opt-in shared ownership through library types `Rc<T>` (single-thread)
  and `Arc<T>` (thread-safe, finalized in `concurrency-safety-and-async-io`),
  not as the default.

This change owns ownership/move semantics and `Drop`; it does **not** introduce
a garbage collector.

## What changes

- ADDED: `Drop` trait and compiler-inserted drop glue.
- ADDED: move/use-after-move checking with a stable diagnostic.
- ADDED: auto-drop behavior for runtime resource types.
- ADDED: `Rc<T>` opt-in shared-ownership library type.
- MODIFIED (additive): existing `.free()/.drop()/.close()` methods become
  idempotent no-ops-after-drop rather than required calls; double manual release
  is defined, not undefined.

## Non-goals

- A tracing or RC-cycle garbage collector (explicitly rejected; see umbrella
  `design.md` Decision 1).
- Removing the explicit release methods in this change (a later, separate change
  may deprecate them once the safe surface is proven).
- `Arc<T>` + thread-safety proofs (delegated to `concurrency-safety-and-async-io`).
