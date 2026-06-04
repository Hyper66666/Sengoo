## 1. Baseline And Syntax

- [x] 1.1 Validate this change with `openspec validate try-and-match-ergonomics --strict`.
- [x] 1.2 Inventory current parser/typeck/lowering match tests and add regression tests for already-shipped match behavior.
- [x] 1.3 Confirm grammar in `design.md` against current parser tokenization before implementation.

## 2. Question-Mark And Try Blocks

- [x] 2.1 Add parser tests for postfix `?`, nested `?`, `try {}` expression blocks, and invalid positions.
- [x] 2.2 Add type-check tests for `Result<T, E>` propagation, `Option<T>` propagation, cross-propagation rejection, error-type mismatch rejection, and `main` restrictions.
- [x] 2.3 Implement lowering for `?` as early return from the nearest compatible function, closure, async function, or `try {}` block.
- [x] 2.4 Add runtime/codegen tests for success and failure paths, including nested calls and generic functions.

## 3. Match Stabilization

- [x] 3.1 Add parser tests for literal, wildcard, binding, enum, tuple-like enum, struct, field shorthand, or-pattern, and guarded arms.
- [x] 3.2 Add type-check tests for binding scope, arm result unification, guard boolean requirements, or-pattern binding compatibility, and unreachable arms.
- [x] 3.3 Add exhaustiveness tests for enums, `Option`, `Result`, literals where supported, guards, and wildcard fallback.
- [x] 3.4 Update lowering/codegen so accepted patterns run correctly without changing existing successful match behavior.

## 4. Diagnostics And LSP

- [x] 4.1 Add stable diagnostic codes and source ranges for invalid `?`, non-exhaustive match, unreachable arm, guard type mismatch, and binding-scope misuse.
- [x] 4.2 Add JSON diagnostic coverage for the new codes.
- [x] 4.3 Add LSP tests for diagnostics and simple quick fixes such as adding `_ => ...` for non-exhaustive matches where safe.

## 5. Verification

- [x] 5.1 Run `cargo fmt --check`.
- [x] 5.2 Run `cargo test -p sengoo-compiler match -- --nocapture`.
- [x] 5.3 Run `cargo test -p sengoo-compiler try -- --nocapture`.
- [x] 5.4 Run `cargo test -p sgc match -- --nocapture`.
- [x] 5.5 Run `cargo test -p sglsp match -- --nocapture`.
- [x] 5.6 Run `sgc check/build/run` against success and failure examples for `?`, `try {}`, and match.

## Done Definition

- [x] The accepted grammar in `design.md` matches implemented parser behavior.
- [x] `?` supports `Result` and `Option` only in compatible contexts.
- [x] `try {}` scopes propagation and returns an explicit `Result` or `Option`.
- [x] Match supports the accepted pattern forms, guards, binding scopes, exhaustiveness checks, and unreachable-arm diagnostics.
- [x] Existing match examples continue to compile.

## Archive Gate

- [x] `openspec validate try-and-match-ergonomics --strict` passes.
- [x] `openspec validate --all --strict` passes.
- [x] Parser/typeck/lowering/codegen/LSP verification commands above pass or have documented, accepted platform skips.
