## 1. WASM backend

- [~] 1.1 Lower scalar MIR to WebAssembly with a direct emitter producing
  `.wasm` modules. Covered today: integer/bool scalar functions, calls,
  branches, switch/goto, phi, and recursion. Deferred: aggregates, heap,
  stdlib/FFI, async, and WASI imports.
- [~] 1.2 Define a WASI-based host-interface subset for stdlib (io/env/time/file
  where the sandbox allows); document unsupported host APIs. Current slice
  documents the capability matrix and rejects unsupported stdlib/FFI with
  `docs/portable-targets.md`; real WASI host imports remain open.
- [~] 1.3 Tests: compile and run representative programs under a WASM runtime
  (e.g. wasmtime) in CI. Current CI runs scalar `.wasm` generation and executes
  it with Node when available; WASI runtime coverage remains open.

## 2. Bytecode VM

- [x] 2.1 Define a portable bytecode format and instruction set for scalar MIR
  (`SGB1` version 1).
- [~] 2.2 Define the VM value/heap model consistent with ownership + `Drop`.
  Current VM is scalar-only with no heap objects; heap/drop opcodes remain open.
- [~] 2.3 Implement the interpreter and a stdlib bridge. Current interpreter
  executes scalar internal calls and rejects stdlib/FFI with documented
  diagnostics; stdlib bridge remains open.
- [~] 2.4 `sgc run` clang-free mode using the VM; measure startup vs native.
  Clang-free execution is implemented and tested with `PATH` removed; startup
  measurement remains open.
- [~] 2.5 Tests: core conformance programs run identically on the VM. Current
  coverage proves scalar branching/calls and recursion; aggregate/stdlib core
  cases remain open pending heap and bridge support.

## 3. Target selection and matrix

- [~] 3.1 `sgc build --target {native,wasm,bytecode}`. `build` supports all
  three; `run --target bytecode` is implemented and `run --target wasm` is
  intentionally build-only until a bundled WASM runner exists.
- [x] 3.2 Per-target capability matrix doc (which stdlib areas work per target).
- [ ] 3.3 Run `openspec validate wasm-and-bytecode-backends --strict`.

## Verification

- WASM programs run under the chosen WASM runtime in CI (task 1.3)
- Core conformance suite passes on the bytecode VM (task 2.5)
- `sgc build --target` produces artifacts for each target
