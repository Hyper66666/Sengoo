## 1. Entry and emitter decision

- [x] 1.1 Confirm coordinator entry gate, MIR semantic ABI version, portable
  runtime ABI version, and explicit wasm32 pointer width.
- [x] 1.2 Promote/replace/discard decision for the direct emitter (recorded in
  `docs/architecture/wasm-emitter-decision.md`).
- [x] 1.3 Freeze experimental scalar contract in design + canonical
  `openspec/specs/wasm-backend` (no production Drop/WASI claim).

## 2. Experimental scalar backend

- [x] 2.1 ABI boundary: pure-core module, custom ABI section, **`main : () -> i64`**
  only (MIR conversion + type/export artifact validation).
- [x] 2.2 Scalar/control-flow/call lowering; aggregates, heap ownership,
  Load/Store/AddrOf, Ref/Ptr/Future, FFI/stdlib fail closed with
  `unsupported-target-capability`.
- [x] 2.3 Module validation, unsigned div/rem/shr/compare, reject unknown ABI
  versions before host execution.
- [x] 2.4 Guardrails: max module size, wall-clock run timeout; no native fallback.

## 3. CLI, tests, docs

- [x] 3.1 `sgc build/run --target wasm` (Node/wasmtime, `SENGOO_WASM_RUNTIME`).
- [x] 3.2 Portable target suite (scalar, signedness, ABI tamper, parameterized
  `main`, aggregates/stdlib negatives).
- [x] 3.3 Docs + SUPPORT_MATRIX: experimental scalar / deferred production.
- [x] 3.4 `openspec validate wasm-backend-v1 --strict` and
  `openspec validate --all --strict`.

## 4. Explicitly out of archive scope (successor work)

These are **not** incomplete acceptance tasks for experimental scalar archive.
They require one or more future OpenSpec changes:

- WASI args/env/stdout/stderr/time/file imports + tests
- Ownership/Drop and aggregate/heap lowering
- Runtime memory/output ceilings beyond module size + wall-clock timeout
- Windows + Unix CI execution matrix for `.wasm`

## Archive gate (experimental scalar) — met

- `main: () -> i64` enforced at MIR and artifact layers
- Signedness + ABI version checks + fail-closed memory ops tested
- Spec/docs/matrix do not claim production WASI/Drop
- Strict OpenSpec validation green
