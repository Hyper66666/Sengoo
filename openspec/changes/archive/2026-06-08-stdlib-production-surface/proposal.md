## Why

Internal programs still depend on `ffi_buffer_*` and handle-heavy stdlib APIs
even though owned `String` exists. This child change owns the canonical stdlib
deltas for Pillar 1 of `six-pillar-gap-closure`.

## What Changes

- Add additive `_string` helpers and owned `String` returns.
- Add `Vec<String>`, `StringMapString`, larger JSON cap, recursive IO, process
  pipes/background, and sync fd IO.
- Keep existing Buffer-based helper names source-compatible.

## Capabilities

### New Capabilities

- None. Canonical updates live in modified capabilities below.

### Modified Capabilities

- `stdlib-mainstream-usability`: production helpers, collections, recursive IO,
  process pipes/background, sync fd IO, Buffer compatibility rule.
- `owned-string-text`: stdlib return ABI for owned `String` helpers.

## Impact

- `tools/stdlib/*.sg`, `tools/stdlib/runtime_*.c`, `runtime_shared.h`
- `tools/sglsp`, `examples/realworld`, stdlib examples
- Parent umbrella: `six-pillar-gap-closure` Pillar 1

## Prerequisites

- Archive or defer overlapping active changes (`stdlib-next-usability-wave`,
  `stdlib-breadth-mainstream`) before editing the same canonical requirements.
