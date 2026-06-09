## Scope

Child change for `six-pillar-gap-closure` Pillar 1. Public API names and
ownership semantics are frozen in the `stdlib-mainstream-usability` and
`owned-string-text` delta specs.

## Principles

- Additive APIs only; Buffer helpers keep current names.
- Symlinks are not followed by default in recursive helpers.
- Resource limits live in `runtime_shared.h`.

## Runtime layout

- Collections/walk: `runtime_collections.c` and/or `runtime_walk.c`
- Pipes/background: `runtime_process.c` with generation-checked handles

## Verification

- stdlib/native tests, `sglsp` signatures, realworld examples without `ffi_buffer_*`
