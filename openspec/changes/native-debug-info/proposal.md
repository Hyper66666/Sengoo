## Why

Sengoo emits no debug metadata at all: there are zero `!dbg` locations or DI
nodes anywhere under `compiler/src/`, so `clang` produces binaries without
DWARF (POSIX) or CodeView (Windows) line information for Sengoo sources.
`docs/debugging-native.md` (Pillar 6 of `six-pillar-gap-closure`) therefore
documents attaching a debugger to a symbol-less artifact: developers step
through assembly, not Sengoo source. Breakpoints on source lines and
source-level stepping are table stakes for every mainstream language; their
absence is the single largest day-one adoption blocker identified by the
`mainstream-adoption-gap-closure` umbrella (Pillar A).

## What Changes

- Add a `-g` / `--debug-info` flag to `sgc build` and `sgc run` that emits
  LLVM debug-info metadata (DI compile units, subprograms, statement
  locations) in the textual IR path, so `clang` produces DWARF on POSIX and
  CodeView on Windows.
- Pin the v1 debug surface to function names, file/line locations,
  breakpoints, and stepping; function parameters as `DILocalVariable` are a
  stretch subset, and full local-variable inspection is out of scope and
  recorded in the support matrix.
- Keep the default pipeline byte-identical: without `-g` no debug metadata
  is emitted, and debug artifacts use a distinct artifact-cache fingerprint
  dimension so `-g` and non-`-g` outputs never alias.
- Prove debug builds preserve program semantics: the pinned conformance
  examples compile, link, and run with unchanged results under `-g`.
- Upgrade `docs/debugging-native.md` to source-level workflows with
  validated lldb (Linux) and WinDbg/cdb (Windows) transcripts.
- Add a new `SUPPORT_MATRIX.md` row for source-level debugging with proof
  links.

## Capabilities

### New Capabilities

- `native-debug-info`: debug-metadata emission, the `-g` enablement policy,
  source-line breakpoint/stepping guarantees, cache separation, and the
  debugger documentation contract.

### Modified Capabilities

- None. Existing codegen capabilities are unchanged when `-g` is absent;
  `codegen-ir-correctness-and-gate` (active) owns IR type consistency and
  must be archived before this change merges codegen-affecting work — it is
  recorded as an explicit blocker.

## Impact

- `compiler/src/codegen/` (DI metadata emission in the textual IR path),
  `compiler/src/` span plumbing where line/column information is not yet
  threaded to codegen.
- `tools/sgc/` (`-g` flag, cache fingerprint dimension in
  `tools/sgc/src/cache.rs`, passing `-g` to `clang` link step).
- `docs/debugging-native.md`, `examples/realworld/SUPPORT_MATRIX.md`,
  `docs/language-features.md` (flag documentation).
- Parent umbrella: `mainstream-adoption-gap-closure` (Pillar A).
- Blocker: `codegen-ir-correctness-and-gate` must be archived first.

## Non-Goals

- No debug-adapter (DAP) or IDE debug UI; native debuggers only.
- No full local-variable, struct-field, or enum-payload inspection in v1.
- No change to default (non-`-g`) emission, optimization behavior, or the
  Cranelift fast path.
- No pretty-printers, expression evaluation, or Sengoo-aware debugger
  scripting.
