## Why

The three P0 child changes are functionally complete, and
`first-class-strings-and-formatting` is already archived, but the P0 gate in
`language-maturity-roadmap` cannot close because deferred work is still
recorded inside `automatic-memory-management` and `generics-and-trait-system`:

- `dyn Trait` vtables have method slots but no `drop` slot or size/align
  metadata, so dropping a `dyn` value cannot run the concrete `Drop`
  (generics tasks 3.2/3.4).
- Dynamic dispatch only covers single-trait `&self` receivers; multi-trait
  `dyn A + B`, value/`&mut self` receivers, and `Box<dyn Trait>` are deferred
  (generics task 3.3).
- Derives reference a `Hasher` object protocol that does not exist yet; the
  matching `Formatter` protocol shipped with the strings change, so `Hasher`
  is the remaining protocol gap (generics task 5.2).
- Borrowed-view escape analysis covers returns, tails, aggregates, and branch
  tails, but not flow-sensitive tracking through reassignment chains (strings
  archive follow-up).

## Proposal

Close the P0 gate by landing the deferred guarantees and archiving the two
remaining P0 children.

- **`dyn` Drop metadata**: extend per-`(trait, concrete)` vtables with a
  `drop` slot plus size/align entries, and make dropping a `dyn` value invoke
  the concrete `Drop` through the vtable.
- **`dyn` dispatch surface**: support `&mut self` receivers through dyn
  dispatch; document (or implement behind a follow-up) multi-trait
  `dyn A + B` and `Box<dyn Trait>` with stable diagnostics until they land.
- **`Hasher` object protocol**: define `Hasher` in the stdlib with
  `write_i64`/`write_str`/`finish`, and let `impl Hash` define
  `hash_into(&self, h: &mut Hasher)` with a compiler-synthesized `hash()`
  bridge, mirroring the shipped `Formatter` protocol.
- **Flow-sensitive borrowed views**: track `&str` views through local
  reassignment chains so re-bound views still report `borrow-escapes-scope`
  and `cannot-move-borrowed`.
- **Gate closure**: archive `automatic-memory-management` and
  `generics-and-trait-system`, tick roadmap items 1.1/1.2, and update
  `examples/realworld/SUPPORT_MATRIX.md` rows moved by P0.

## What changes

- ADDED: vtable `drop`/size/align metadata and dyn-value drop semantics.
- ADDED: `&mut self` dyn dispatch; stable diagnostics for still-unsupported
  `dyn A + B` and `Box<dyn Trait>`.
- ADDED: stdlib `Hasher` type and the `hash_into` protocol bridge.
- ADDED: flow-sensitive borrowed-view tracking through reassignments.
- MODIFIED: umbrella roadmap P0 gate marked complete after both children
  archive.

## Non-goals

- Recoverable unwinding/panic semantics (dyn drop uses the existing abort and
  scope-exit paths).
- Full NLL borrow checking; only local reassignment chains of borrowed views
  are in scope.
- `Box<T>` as a general heap-allocation surface; only the `dyn`-related
  diagnostic contract is covered here.
