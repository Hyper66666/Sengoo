## Scope

Child change for `six-pillar-gap-closure` Pillar 4. All acceptance rules are
frozen in this change's spec; no dependency on umbrella prose at archive time.

## Attribute matrix

See `specs/language-surface-expansion/spec.md` for the full matrix and cfg/deprecated
rules.

## Class headers

- First resolved class path becomes the sole base.
- First resolved trait path means trait-only header with no base.
- Class after trait or second class base is an error.

## FFI

- Dynamic native i64 call arity `0..=8` only.
- Aggregates, owned `String`, and callback expansion remain unsupported.

## Out of scope

- Async frame restriction cleanup belongs to `async-reactor-futures`, not this change.
