# Portable Targets

Sengoo currently has three build target families:

| Target | Artifact | Host toolchain | Support tier | Current capability |
| --- | --- | --- | --- | --- |
| `native` | platform executable | clang/LLVM or cached native artifacts | **production** | Full supported stdlib/runtime surface |
| `wasm` | `.wasm` | Node or wasmtime | **experimental scalar** | Scalar MIR, wasm32 width, signedness-correct integer ops, ABI-validated modules, `sgc run --target wasm` |
| `bytecode` | `.sgbc` | none at run time | **experimental prototype (production VM NO-GO)** | Scalar MIR subset only |

Native production semantics remain the differential oracle. Unsupported features
fail with `unsupported-target-capability` and never fall back to native.

## WASM (experimental scalar)

See also:

- `docs/architecture/wasm-emitter-decision.md`
- `docs/wasm-wasi-profile.md`
- `runtime/abi/portable_runtime_abi_v1.json`

```bash
sgc build input.sg --target wasm -o app.wasm
sgc run input.sg --target wasm
```

### Guarantees

- Frontend uses `wasm32-unknown-unknown` (32-bit `usize`/`isize`).
- Modules embed MIR + portable runtime ABI versions; unknown versions are
  rejected **before** host execution.
- Unsigned integer div/rem/shr/compare use unsigned opcodes/semantics.
- Load/Store/AddrOf and non-scalar types fail closed (not silent Move).

### Non-guarantees (deferred)

- Owned String/Vec/user Drop in linear memory
- WASI args/env/stdout/stderr/time/file imports
- Windows+Unix CI matrix for portable smoke (Ubuntu only today)

## Bytecode prototype

```bash
sgc build input.sg --target bytecode -o app.sgbc
sgc run input.sg --target bytecode
```

Production VM cancelled: `docs/bytecode-vm-value-review.md`.
