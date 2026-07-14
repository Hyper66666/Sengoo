# WASM / WASI Profile (experimental scalar v1)

Pinned profile name: **`sengoo-wasm32-scalar-experimental-v1`**

> **Not production WASM.** This profile documents the experimental scalar
> backend only. Owned String/Vec Drop and WASI host imports are deferred and
> MUST NOT be claimed as Supported production capabilities.

## Target triple and ABI

| Item | Value |
| --- | --- |
| Frontend triple | `wasm32-unknown-unknown` |
| Pointer width | 32-bit (`isize`/`usize`) |
| MIR semantic ABI | `1` |
| Portable runtime ABI | `1` (`runtime/abi/portable_runtime_abi_v1.json`) |
| Module export | `main : () -> i64` |
| Emitter | Direct MIR→WASM (`docs/architecture/wasm-emitter-decision.md`) |
| Support tier | **Experimental scalar** |

## Supported program surface

- Scalar MIR: `i64`/`u*`/`bool`/unit value types, internal calls, recursion,
  branches, loops, switch, phi, integer/boolean arithmetic with correct
  signedness for div/rem/shr/compare.
- Compile-time rejection (`unsupported-target-capability`) for:
  - Load / Store / AddrOf
  - Ref / Ptr / Future types
  - aggregates and heap ownership
  - dynamic FFI and host stdlib externs
  - async / reactor I/O

## WASI host import subset

**Not implemented.** Experimental modules are pure core WebAssembly (no imports).

Forward allowlist for a future production change (not a support claim):

| Capability | Import candidates | Current status |
| --- | --- | --- |
| args | `args_sizes_get`, `args_get` | Deferred |
| env | `environ_sizes_get`, `environ_get` | Deferred |
| stdout/stderr | `fd_write` | Deferred |
| time | `clock_time_get` | Deferred |
| sandboxed file IO | `path_open`, `fd_read`, `fd_close` | Deferred |

## Resource limits (enforced vs documented)

| Limit | Enforcement |
| --- | --- |
| Module size ≤ 4 MiB | Enforced in `validate_wasm_module` |
| Embedded ABI versions | Enforced on build and `sgc run` of `.wasm` |
| Wall-clock run timeout (10s) | Enforced around Node/wasmtime process |
| wasmtime fuel | Best-effort (`--fuel` when CLI accepts it) |
| Multi-OS CI matrix | **Not yet** — Ubuntu portable smoke only |

## CLI

```bash
sgc build program.sg --target wasm -o app.wasm
sgc run program.sg --target wasm
SENGOO_WASM_RUNTIME=wasmtime sgc run program.sg --target wasm
```
