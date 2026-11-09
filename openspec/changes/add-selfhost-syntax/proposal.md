# SG syntax and formatter

## Why

The self-hosting foundation supplies source bytes and checked IDs but no SG
language frontend. M2 requires an actual syntax tree and deterministic lexical
behavior before semantic analysis can be ported.

## What changes

Implement a trivia-preserving SG lexer, literal decoding, an append-only AST
with declaration/statement/expression/type/pattern edges, recursive descent and
Pratt expression parsing. Add a formatter which validates parsed input and
preserves the nontrivia token sequence. Compare the Windows supported corpus
and malformed syntax against the existing Rust reference.

This child owns M2 syntax and formatter behavior. Foundation/seed identity
remains owned by `add-selfhost-foundations`. Semantic analysis and code generation
remain later children. Raw attribute/macro syntax does not count as equivalent
macro expansion, cfg filtering, derive or protocol synthesis.

## Impact

Implementation modules are under `selfhost/src/` and native checks under
`selfhost/tests/`. The stage0 toolchain still builds them. No default compiler,
language edition, published diagnostic schema or existing formatter is replaced.
