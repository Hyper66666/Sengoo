## 1. Entry and emitter decision

- [ ] 1.1 Confirm coordinator entry gate, MIR semantic ABI version, portable
  runtime ABI version, and explicit wasm32 pointer width.
- [ ] 1.2 Promote, replace, or discard the existing direct-emitter prototype
  after an LLVM-target comparison over the representative corpus; record
  artifacts, diagnostics, compile time, and maintenance cost.
- [ ] 1.3 Choose one emitter and update this design before production code.

## 2. Target ABI and code generation

- [ ] 2.1 Define wasm32 layout, function ABI, imports, globals, memory,
  vtables, panic/trap, and runtime ABI version sections.
- [ ] 2.2 Lower scalar/control-flow/call/aggregate/string/generic/drop MIR needed
  by the conformance corpus.
- [ ] 2.3 Validate every produced module and reject unsupported MIR with stable
  diagnostics.

## 3. WASI stdlib subset

- [ ] 3.1 Implement and document args/env/stdout/stderr/time plus sandboxed file
  IO for the pinned WASI profile.
- [ ] 3.2 Add compile-time negatives for process, dynamic FFI, unsupported net,
  and other host-only modules.
- [ ] 3.3 Enforce test/runtime memory, time/fuel, and output limits.

## 4. CLI, tests, and docs

- [ ] 4.1 Promote or replace the experimental `sgc build --target wasm` path and
  add explicit `sgc run --target wasm` runtime selection.
- [ ] 4.2 Run core differential conformance plus String/Vec/Drop and error/trap
  cases under the pinned runtime.
- [ ] 4.3 Add CI module validation and execution on Windows plus one Unix host.
- [ ] 4.4 Update target capability matrix and user documentation.
- [ ] 4.5 Run `openspec validate wasm-backend-v1 --strict` and all strict.
