## 1. Preparation

- [x] 1.1 Run `openspec validate language-surface-expansion --strict`.
- [x] 1.2 Run `openspec validate --all --strict`.

## 2. Implementation

- [x] 2.1 Implement attribute matrix from spec.
- [x] 2.2 Parse and typecheck class header trait lists.
- [x] 2.3 Widen dynamic native i64 FFI arity to `0..=8`.

## 3. Verification

- [x] 3.1 `cargo test -p sengoo-compiler attributes` (or targeted parser/typeck tests)
- [x] 3.2 `cargo test -p sengoo-compiler class` / trait dispatch tests
- [x] 3.3 `cargo test -p sengoo-runtime ffi`
- [x] 3.4 `cargo test -p sglsp` diagnostic parity for deprecated/cfg where applicable (64/64)

## Archive Gate

- [x] `openspec validate language-surface-expansion --strict` passes.
- [x] `openspec validate --all --strict` passes.
- [x] This change archives independently of `async-reactor-futures`.
- [x] Verification commands in section 3 pass.
