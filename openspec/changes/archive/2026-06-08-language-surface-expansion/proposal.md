## Why

Internal libraries hit explicit compiler limits on attributes, class header trait
lists, and FFI arity. This child change owns the new canonical
`language-surface-expansion` capability for Pillar 4.

## What Changes

- Attribute matrix for `derive`, `cfg(target_os)`, and `deprecated`.
- Class header trait lists with base/trait disambiguation rules.
- Dynamic native i64 FFI arity `0..=8`.

## Capabilities

### New Capabilities

- `language-surface-expansion`: attributes, class headers, FFI arity widen.

### Modified Capabilities

- None in canonical `openspec/specs/` today.

## Impact

- `compiler/src/parser/`, `compiler/src/typeck/`, `runtime/src/reflect/runtime_ffi.rs`
- Parent umbrella: `six-pillar-gap-closure` Pillar 4

## Independence

- Async frame restriction cleanup is owned by `async-reactor-futures`, not this change.
