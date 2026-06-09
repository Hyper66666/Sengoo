## Why

The initial P4 language work has landed the large user-visible pieces:
attributes, class header trait lists, dynamic i64 FFI arity, `?`, `try {}`,
match diagnostics, and owned `String`. The remaining mainstream-readiness gap is
smaller but still user-facing: several compiler phases still advertise narrow
implementation limits or generic diagnostics for forms that mainstream users
will try next.

## What Changes

- Create a follow-up owner for additive language surface relaxations that remain
  after `language-surface-expansion`, archived `try-and-match-ergonomics`, and
  archived `owned-string-text`.
- Pin candidate relaxations and rejection parity for parser/typeck/lowering,
  async frames, FFI signatures, attributes, match/try diagnostics, and LSP.
- Require negative tests for every still-rejected form so unsupported language
  shapes stay intentional and diagnosable.
- Gate any source-incompatible cleanup on migration documentation before the
  cleanup can be implemented.

## Capabilities

### New Capabilities

- `language-default-polish`: additive language default polish, diagnostic
  parity, and migration gates for future breaking cleanup.

### Modified Capabilities

- None in canonical `openspec/specs/` today.

## Impact

- OpenSpec only in this worker task.
- Future implementation may touch `compiler/src/parser/`,
  `compiler/src/typeck/`, `compiler/src/hir/`, `compiler/src/mir/`, `tools/sglsp/`,
  and diagnostics tests.
- Parent umbrella: `mainstream-default-readiness` P4.

## Non-Goals

- No compiler/runtime implementation in this change creation task.
- No duplicate ownership of delivered `?`, `try {}`, match exhaustiveness,
  owned `String`, class header trait lists, initial attribute matrix, or
  dynamic native i64 FFI arity `0..=8`.
- No implicit FFI aggregate, owned `String`, callback, or generic extern support
  without a pinned additive scenario and negative compatibility tests.
