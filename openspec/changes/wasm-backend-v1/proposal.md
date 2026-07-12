## Why

After the mainstream-default native path stabilizes, WASM provides a sandboxed,
portable deployment target for CLI-like WASI programs and future browser or
component-model work. It must preserve Sengoo ownership/Drop semantics and fail
clearly where native stdlib assumptions do not apply.

## Proposal

- Add a `wasm32-wasi`-class target producing validated `.wasm` modules.
- Select LLVM-to-WASM or a direct MIR emitter through a bounded implementation
  spike, then record the decision before production code.
- Define a versioned WASI host import layer for supported io/env/args/time/file
  capabilities.
- Reuse the native conformance corpus and add target-specific resource,
  unsupported-capability, ownership, and deterministic execution tests.
- Expose target selection through `sgc build/run --target wasm` with explicit
  runtime selection.

## Impact

- Compiler/sgc backend selection, WASM emitter/link flow, WASI stdlib bridge,
  target capability matrix, CI runtime, examples, and tests.
- Parent coordinator: `wasm-and-bytecode-backends`.
- Begins only after the coordinator entry gate passes.

## Non-goals

- Browser DOM bindings, JavaScript framework integration, component model, or
  interface types in v1.
- Threads, dynamic native FFI, process spawning, or transparent native fallback.
- Bytecode VM implementation.
