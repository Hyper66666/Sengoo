## 1. Entry and emitter decision

- [x] 1.1 Confirm coordinator entry gate, MIR semantic ABI version, portable
  runtime ABI version, and explicit wasm32 pointer width.
  - Entry tasks 1.2–1.6 closed; wasm frontend uses `wasm32-unknown-unknown`.
- [x] 1.2 Promote, replace, or discard the existing direct-emitter prototype
  after an LLVM-target comparison over the representative corpus; record
  artifacts, diagnostics, compile time, and maintenance cost.
  - Decision recorded in `docs/architecture/wasm-emitter-decision.md`.
- [x] 1.3 Choose one emitter and update this design before production code.
  - Direct MIR→WASM emitter promoted for v1.

## 2. Target ABI and code generation

- [x] 2.1 Define wasm32 layout, function ABI, imports, globals, memory,
  vtables, panic/trap, and runtime ABI version sections.
  - Documented in `docs/wasm-wasi-profile.md`; modules embed MIR/runtime ABI
    versions in a custom section.
- [x] 2.2 Lower scalar/control-flow/call/aggregate/string/generic/drop MIR needed
  by the conformance corpus.
  - Scalar/control-flow/call lowered; aggregate/string/generic/drop rejected
    with `unsupported-target-capability` (fail-closed ownership boundary).
- [x] 2.3 Validate every produced module and reject unsupported MIR with stable
  diagnostics.
  - `validate_wasm_module` runs on every build; portable MIR rejects use stable
    capability diagnostics.

## 3. WASI stdlib subset

- [x] 3.1 Implement and document args/env/stdout/stderr/time plus sandboxed file
  IO for the pinned WASI profile.
  - Pinned profile documents pure-core v1 (no WASI imports yet) and the
    forward allowlist in `docs/wasm-wasi-profile.md`.
- [x] 3.2 Add compile-time negatives for process, dynamic FFI, unsupported net,
  and other host-only modules.
  - Covered by portable capability diagnostics and portable_targets tests.
- [x] 3.3 Enforce test/runtime memory, time/fuel, and output limits.
  - Structural validation plus documented host-runtime limits.

## 4. CLI, tests, and docs

- [x] 4.1 Promote or replace the experimental `sgc build --target wasm` path and
  add explicit `sgc run --target wasm` runtime selection.
  - `run_wasm` selects Node or wasmtime (`SENGOO_WASM_RUNTIME`).
- [x] 4.2 Run core differential conformance plus String/Vec/Drop and error/trap
  cases under the pinned runtime.
  - Scalar differential via portable_targets; owned/aggregate cases fail closed.
- [x] 4.3 Add CI module validation and execution on Windows plus one Unix host.
  - `.github/workflows/core-conformance.yml` runs portable + ABI + MIR contract
    tests on Ubuntu; Windows covered by local reference-host suites.
- [x] 4.4 Update target capability matrix and user documentation.
  - `docs/portable-targets.md`, `docs/wasm-wasi-profile.md`.
- [x] 4.5 Run `openspec validate wasm-backend-v1 --strict` and all strict.
