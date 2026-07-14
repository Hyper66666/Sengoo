# WASM Emitter Decision (wasm-backend-v1)

Date: 2026-07-15  
Decision: **promote the direct MIR→WASM scalar emitter** as the production
path for `sgc build/run --target wasm` v1.

## Comparison corpus

Programs:

1. scalar branch (`choose(40) -> 42`)
2. recursive calls (`fib(7)`)
3. unsupported stdlib import (`std::time`) — diagnostic-only
4. owned `String` — diagnostic-only reject

| Path | Artifact validity | Runtime | Compile packaging | Maintenance |
| --- | --- | --- | --- | --- |
| Direct MIR→WASM emitter in `tools/sgc/src/portable_backends.rs` | Valid core module + structural validator; Node/wasmtime execute `main` | No host C/clang; runs via Node or wasmtime | Ships inside `sgc` | One small emitter; ABI metadata custom section |
| LLVM-text → `clang --target=wasm32-wasi` | Depends on host clang/wasi-sysroot and full stdlib link surface | WASI runtime required for any host call | Couples portable path to native toolchain packaging | Dual IR maintenance + target-triple drift |

## Evidence on the Windows reference host

- `cargo test -p sgc --test portable_targets` builds and validates `.wasm`
  modules and executes the scalar program with Node when present.
- Out-of-range `usize` on wasm32 fails in the frontend before emission.
- Unsupported stdlib/FFI/owned-heap surfaces fail with
  `unsupported-target-capability` and never fall back to native.

## Chosen path

Promote the **direct emitter** for v1:

1. It keeps portable builds free of clang/wasi-sdk install requirements.
2. It already consumes MIR semantic ABI v1 + portable runtime ABI v1 and
   enforces wasm32 pointer width.
3. LLVM-to-WASM remains a future option only if aggregate/WASI coverage
   exceeds the direct emitter’s maintainability budget.

The previous prototype bytes are **not** a compatibility promise; v1 modules
embed MIR/runtime ABI versions in a custom section
(`sengoo.portable_runtime_abi`) and are re-validated on every build.
