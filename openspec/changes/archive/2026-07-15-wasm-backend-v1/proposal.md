## Why

After the mainstream-default native path stabilizes, Sengoo still needs a
sandboxed portable target for simple scalar programs and a hardened boundary
for unsupported capabilities. The current backend already proves a direct
MIR-to-WASM path for pure core modules, but it does not yet implement WASI
host imports, ownership/Drop lowering, or broader runtime hardening. This
change therefore narrows v1 to the experimental scalar surface and makes the
unsupported boundary explicit.

## Proposal

- Keep the direct MIR-to-WASM emitter as the experimental
  `sgc build --target wasm` / `sgc run --target wasm` path for scalar
  control-flow and call programs that validate as core `.wasm` modules.
- Keep the backend fail closed: aggregates, heap ownership/Drop,
  `Load`/`Store`/`AddrOf`, FFI, and unsupported stdlib or host imports continue
  to fail with `unsupported-target-capability`.
- Preserve the hardening that is already implemented: embedded MIR/runtime ABI
  versions, pre-run ABI validation, unsigned integer opcode selection, module
  size validation, and wall-clock timeout around runtime execution.
- Reopen follow-up work instead of treating it as complete: WASI host imports,
  ownership/Drop lowering, memory or output limit enforcement, and Windows plus
  Unix CI execution coverage.
- Keep the surface experimental only; current prototype artifacts still carry no
  broader compatibility or production support promise.

## Impact

- Compiler/sgc portable backend selection, ABI and validation boundaries,
  runtime timeout behavior, target capability matrix, and user-facing
  experimental WASM documentation.
- Parent coordinator: `wasm-and-bytecode-backends`.
- Begins only after the coordinator entry gate passes.

## Non-goals

- WASI host import support in v1.
- Ownership/Drop, aggregate, or heap-backed lowering in v1.
- Memory or output limit enforcement beyond the current module-size validation
  and wall-clock timeout.
- Claiming Windows plus Unix CI execution coverage as complete for this change.
- Browser DOM bindings, JavaScript framework integration, component model, or
  interface types in v1.
- Threads, dynamic native FFI, process spawning, or transparent native fallback.
- Bytecode VM implementation.
