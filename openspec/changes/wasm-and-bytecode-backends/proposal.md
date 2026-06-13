## Why

The language spec (`Sengoo_Language_Specification.md` §1.4) promises three code
generation backends — LLVM, a bytecode VM, and WASM — but only the textual-LLVM
(+clang) and Cranelift paths are implemented. Two consequences:

- **No WASM target**, so Sengoo cannot run in browsers or WASM sandboxes, a
  major deployment surface for mainstream languages.
- **No bytecode VM**, so the spec's "fast startup via bytecode" goal and a
  portable, clang-free execution mode are unmet (today AOT requires clang/LLVM
  15+ installed).

This depends on a stable MIR/runtime ABI, so it starts after P0 stabilizes.

## Proposal

- **WASM backend**: lower MIR to WebAssembly (via the existing LLVM path's
  `wasm32` target or a direct emitter), producing `.wasm` modules. Define a
  WASI-based host interface subset for the stdlib (file/io/env/time/process
  where the sandbox allows) and document unsupported host APIs.
- **Bytecode VM**: a portable bytecode format plus an interpreter for fast
  startup and clang-free execution, suitable for scripting and `sgc run` without
  a native toolchain. Define the instruction set, the value/heap model
  (consistent with the ownership/`Drop` model), and the stdlib bridge.
- **Target selection**: `sgc build --target {native,wasm,bytecode}` with a
  documented capability matrix per target.

## What changes

- ADDED: WASM backend emitting `.wasm` + a WASI host-interface subset.
- ADDED: bytecode format + interpreter VM + `sgc run` clang-free mode.
- ADDED: `--target` selection and a per-target capability matrix.

## Non-goals

- WASM component model / interface types (a later proposal); the MVP targets
  core WASM + WASI preview.
- A JIT for the bytecode VM (interpreter first; JIT proposable later).
