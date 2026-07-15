## 1. Entry and emitter decision

- [x] 1.1 Confirm coordinator entry gate, MIR semantic ABI version, portable
  runtime ABI version, and explicit wasm32 pointer width.
  - Entry tasks 1.2-1.6 closed; wasm frontend uses `wasm32-unknown-unknown`.
- [x] 1.2 Promote, replace, or discard the existing direct-emitter prototype
  after an LLVM-target comparison over the representative corpus; record
  artifacts, diagnostics, compile time, and maintenance cost.
  - Decision recorded in `docs/architecture/wasm-emitter-decision.md`.
- [x] 1.3 Choose one emitter and update this design to match the experimental
  scalar contract.
  - Direct MIR-to-WASM emitter remains the only recorded path for v1.

## 2. Implemented experimental scalar backend

- [x] 2.1 Define the experimental wasm32 scalar ABI, export shape, and runtime
  ABI metadata boundary.
  - Experimental profile documented in `docs/wasm-wasi-profile.md`; modules
    export **`main : () -> i64`** only (MIR conversion + artifact type/export
    validation), embed MIR/runtime ABI versions in a custom section, and
    require no host imports. Parameterized `main` is a hard diagnostic.
- [x] 2.2 Lower scalar/control-flow/call MIR and fail closed for unsupported
  ownership or memory features.
  - Scalar/control-flow/call lowered; aggregates, String/Vec/Drop,
    `Load`/`Store`/`AddrOf`, and host-only features reject with
    `unsupported-target-capability`.
- [x] 2.3 Validate every produced module, preserve unsigned semantics, and
  reject unsupported ABI versions before run.
  - `validate_wasm_module` runs on every build; unsigned div/rem/shr/compare
    use the correct WASM opcodes; `sgc run --target wasm` rejects unknown ABI
    versions before execution.
- [x] 2.4 Keep the implemented runtime guardrails honest.
  - Module-size validation and wall-clock timeout are enforced; no hidden
    native fallback is used.

## 3. CLI, tests, and docs aligned to the experimental scope

- [x] 3.1 Keep the experimental `sgc build --target wasm` path and explicit
  `sgc run --target wasm` runtime selection.
  - `run_wasm` selects Node or wasmtime (`SENGOO_WASM_RUNTIME`).
- [x] 3.2 Reuse core scalar conformance and negative tests for the implemented
  boundary.
  - Scalar differential runs in `portable_targets`; owned/aggregate and
    host-only cases fail closed instead of executing.
- [x] 3.3 Update target capability matrix and user documentation without
  claiming production WASI or ownership support.
  - `docs/portable-targets.md` and `docs/wasm-wasi-profile.md` stay aligned
    with the canonical `openspec/specs/wasm-backend/spec.md` boundary.
- [x] 3.4 Run `openspec validate wasm-backend-v1 --strict` and all strict.
  - 2026-07-15: `npx @fission-ai/openspec validate --all --strict` passed
    48/48 specs and changes.

## 4. Reopened follow-up work (not complete in this change)

- [ ] 4.1 Implement WASI host imports for args/env/stdout/stderr/time/file IO
  and cover them with runtime-backed tests.
- [ ] 4.2 Lower ownership/Drop, aggregates, and other heap-backed values instead
  of rejecting them with `unsupported-target-capability`.
- [ ] 4.3 Enforce runtime memory and output limits in code and tests.
  - Documenting future limits is not evidence that the implementation exists.
- [ ] 4.4 Run `.wasm` artifacts in CI on both Windows and Unix hosts.
