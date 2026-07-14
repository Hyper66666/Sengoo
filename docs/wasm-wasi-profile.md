# WASM / WASI Profile (v1)

Pinned profile name: **`sengoo-wasm32-scalar-v1`**

## Target triple and ABI

| Item | Value |
| --- | --- |
| Frontend triple | `wasm32-unknown-unknown` |
| Pointer width | 32-bit (`isize`/`usize` are 32-bit) |
| MIR semantic ABI | `1` (`MIR_SEMANTIC_ABI_VERSION`) |
| Portable runtime ABI | `1` (`runtime/abi/portable_runtime_abi_v1.json`) |
| Module export | `main : () -> i64` (WebAssembly `i64`) |
| Emitter | Direct MIR→WASM (`docs/architecture/wasm-emitter-decision.md`) |

## Supported program surface (v1)

- Scalar MIR: `i64`/`bool`/unit, internal calls, recursion, branches, loops,
  switch, phi, integer/boolean arithmetic.
- Compile-time rejection (stable `unsupported-target-capability`) for:
  - dynamic FFI and host stdlib externs
  - owned `String`, generic collections, aggregates requiring heap layout
  - async functions and reactor I/O
  - process / network / reflection modules

## WASI host import subset

v1 **does not emit WASI imports**. Scalar programs are pure core WebAssembly.

The forward WASI allowlist for a later revision is pinned conceptually to
`wasi_snapshot_preview1` capabilities:

| Capability | Import candidates | v1 status |
| --- | --- | --- |
| args | `args_sizes_get`, `args_get` | Unsupported (compile reject if used via stdlib) |
| env | `environ_sizes_get`, `environ_get` | Unsupported |
| stdout/stderr | `fd_write` (fd 1/2) | Unsupported |
| time | `clock_time_get` | Unsupported |
| sandboxed file IO | `path_open`, `fd_read`, `fd_close` | Unsupported |
| process / dynamic FFI / net | — | Permanently out of scope for this profile |

When those imports are implemented, they must map through portable runtime ABI
host-call IDs (`args_read`, `env_read`, `stdout_write`, …) rather than native C
pointers from `runtime_shared.h`.

## Resource limits (test/runtime)

| Limit | Value | Enforcement |
| --- | --- | --- |
| Module validation | structural section order + type/function/code agreement | `validate_wasm_module` before reporting build success |
| Host runtime | Node.js or wasmtime (`SENGOO_WASM_RUNTIME`) | `sgc run --target wasm` |
| Wall time (wasmtime) | host default; optional future `--wasm-timeout` | documented |
| Output | integer `main` result printed/parsed | runner scripts |
| Memory | core module has no linear memory in v1 scalar emitter | n/a |

## CLI

```bash
sgc build program.sg --target wasm -o app.wasm
sgc run program.sg --target wasm
# or
SENGOO_WASM_RUNTIME=wasmtime sgc run program.sg --target wasm
```
