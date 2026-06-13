## 1. WASM backend

- [ ] 1.1 Lower MIR to WebAssembly (reuse the LLVM `wasm32` target or a direct
  emitter) producing `.wasm` modules.
- [ ] 1.2 Define a WASI-based host-interface subset for stdlib (io/env/time/file
  where the sandbox allows); document unsupported host APIs.
- [ ] 1.3 Tests: compile and run representative programs under a WASM runtime
  (e.g. wasmtime) in CI.

## 2. Bytecode VM

- [ ] 2.1 Define a portable bytecode format and instruction set.
- [ ] 2.2 Define the VM value/heap model consistent with ownership + `Drop`.
- [ ] 2.3 Implement the interpreter and a stdlib bridge.
- [ ] 2.4 `sgc run` clang-free mode using the VM; measure startup vs native.
- [ ] 2.5 Tests: core conformance programs run identically on the VM.

## 3. Target selection and matrix

- [ ] 3.1 `sgc build --target {native,wasm,bytecode}`.
- [ ] 3.2 Per-target capability matrix doc (which stdlib areas work per target).
- [ ] 3.3 Run `openspec validate wasm-and-bytecode-backends --strict`.

## Verification

- WASM programs run under the chosen WASM runtime in CI (task 1.3)
- Core conformance suite passes on the bytecode VM (task 2.5)
- `sgc build --target` produces artifacts for each target
