# Portable Targets

Sengoo currently has three build target families:

| Target | Artifact | Host toolchain | Support tier | Current capability |
| --- | --- | --- | --- | --- |
| `native` | platform executable | clang/LLVM or cached native artifacts | **production** | Full supported stdlib/runtime surface |
| `wasm` | `.wasm` | WebAssembly runtime (Node or wasmtime) | **experimental → v1 scalar** | Scalar MIR subset, wasm32 pointer width, structural module validation, `sgc run --target wasm` |
| `bytecode` | `.sgbc` | none at run time | **experimental prototype (no-go for production VM)** | Scalar MIR subset only; not a product runtime |

Native production semantics remain the differential oracle for portable
backends. Unsupported features fail with diagnostic code
`unsupported-target-capability` (including `target \`…\``) and never fall back
to native execution.

## WASM v1

See:

- `docs/architecture/wasm-emitter-decision.md` — emitter choice (direct MIR→WASM)
- `docs/wasm-wasi-profile.md` — pinned profile, ABI versions, limits, WASI roadmap
- `runtime/abi/portable_runtime_abi_v1.json` — portable runtime contract

```bash
sgc build input.sg --target wasm -o app.wasm
sgc run input.sg --target wasm
```

Frontend lowering always uses `wasm32-unknown-unknown` (32-bit `usize`/`isize`).
Modules embed MIR semantic ABI and portable runtime ABI versions in a custom
section and are structurally validated before build success is reported.

### Supported source shape (wasm v1)

- Scalar MIR: `i64`, `bool`, unit, internal calls, recursion, branches, loops,
  switch, phi, integer/boolean ops.
- `main` exported as WebAssembly `i64`.

### Unsupported (compile-time reject)

- FFI / host stdlib externs (file, process, network, string heap, JSON, …)
- Owned `String`, generic collections, aggregate heap layouts, automatic Drop
  for heap values
- WASI imports (documented for later; not emitted in v1)
- Async / reactor I/O

## Bytecode prototype

```bash
sgc build input.sg --target bytecode -o app.sgbc
sgc run input.sg --target bytecode   # clang-free
```

The `SGB1` format is an experimental spike. The maturity-program value review
recorded a **NO-GO** for a production VM; see
`docs/bytecode-vm-value-review.md`. Do not treat version `1` as a compatibility
promise.

## Forward compatibility

Portable backends consume:

- MIR semantic ABI version (`MIR_SEMANTIC_ABI_VERSION`)
- portable runtime ABI version (`PORTABLE_RUNTIME_ABI_VERSION` /
  `runtime/abi/portable_runtime_abi_v1.json`)

Unknown versions are rejected before emission or execution.
