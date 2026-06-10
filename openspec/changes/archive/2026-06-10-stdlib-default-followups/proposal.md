## Why

`mainstream-default-readiness` P3 requires stdlib additions to be
demand-backed, bounded, and proven by realworld fixtures. Existing stdlib waves
already own or complete status categories, Buffer helpers, JSON handles,
owned-string returns, recursive IO, process control, breadth modules, HTTP, and
TLS. Reopening those surfaces would duplicate active ownership.

The remaining concrete user-facing gap is compression: the realworld support
matrix still marks compression as `Deferred`, while `std::compress` wrappers
exist and runtime gzip/gunzip paths currently report `STATUS_UNSUPPORTED`.
Mainstream package and CLI workflows commonly exchange compressed JSON, logs,
and artifact bundles, so this child change owns the next-wave decision and proof
bar for making compression real instead of leaving an ambiguous placeholder.

## What Changes

- Promote compression from a deferred matrix row to a child-owned
  implementation contract.
- Require gzip/zlib-compatible compression and decompression APIs to define
  resource limits, stable status categories, output ownership, and platform
  behavior before implementation.
- Require a realworld compressed-artifact fixture before claiming support.
- Keep streaming JSON, schema validation, terminal control, file locks,
  long-lived file watchers, Unicode grapheme/locale behavior, async network, and
  broader server/network helpers gated until a fixture-backed follow-up accepts
  them.

## Capabilities

### Modified Capabilities

- `stdlib-mainstream-usability`: Adds demand-backed default-readiness follow-up
  gates for compression and future streaming data helpers.

## Impact

- Future implementation areas: `tools/stdlib/compress.sg`,
  `tools/stdlib/runtime_breadth.c` or a sibling compression bridge,
  `runtime_shared.h`, `tools/sgc/src/stdlib_imports.rs`, `tools/sglsp`,
  `examples/stdlib`, `examples/realworld`, and
  `examples/realworld/SUPPORT_MATRIX.md`.
- Parent umbrella: `mainstream-default-readiness` P3 stdlib thickness.

## Non-Goals

- No stdlib implementation in this change.
- No new dependency is approved by this proposal alone.
- No archive claim based only on `STATUS_UNSUPPORTED` stubs.
- No duplicate ownership of JSON core handles, owned string helpers, recursive
  IO, process capture/background, HTTP/TLS, or breadth modules already owned by
  existing stdlib changes.
- No streaming parser, schema validator, terminal raw mode, file lock, persistent
  watch event stream, Unicode normalization/collation, async network execution,
  or HTTP server expansion without a later OpenSpec update and realworld demand.
