## Why

`Sengoo_Language_Specification.md` is a design draft that is out of sync with the
implementation (it describes RC + cycle GC, JIT, bytecode/WASM backends, and
literal forms that are not implemented), and parts of it are encoding-corrupted.
There is good scattered documentation (`docs/language-features.md`,
`SUPPORT_MATRIX.md`, OpenSpec specs), but no single accurate, versioned language
reference a user can trust. Mainstream languages have an authoritative reference
kept in lockstep with the compiler.

## Proposal

Produce an **authoritative, versioned language reference** that matches the
implemented language and is kept honest by tests.

- A structured reference covering: lexical grammar, types, expressions/
  statements, ownership/borrowing + `Drop`, generics/traits, pattern matching,
  modules/visibility, attributes, FFI, async, and the formatting mini-language.
- Each construct documents status (Supported / subset / unsupported) with a link
  to the proof example or test, replacing the stale draft as the source of truth.
- **Doc-tests**: code blocks in the reference are compiled (and run where
  applicable) by CI so the reference cannot drift from the compiler.
- A versioning policy aligning the reference with toolchain releases, and a
  migration note pointing the old design draft at the new reference.

## What changes

- ADDED: an authoritative, versioned language reference synced to the
  implementation.
- ADDED: CI doc-tests that compile/run the reference's code blocks.
- MODIFIED: the legacy `Sengoo_Language_Specification.md` is marked historical
  and points to the new reference.

## Non-goals

- A formal operational semantics / mechanized proof.
- Tutorials and a learning guide (valuable, but a separate doc effort).
