# Bytecode VM Value Review (go/no-go)

Date: 2026-07-15  
Decision: **NO-GO** for a production bytecode VM in this maturity program.

## Entry gates confirmed

- Coordinator MIR semantic ABI v1 and portable runtime ABI v1 are closed
  (`wasm-and-bytecode-backends` tasks 1.2–1.6).
- Child designs already classify the scalar `SGB1` emitter/interpreter as an
  experimental prototype with no compatibility promise.

## Measured alternatives (Windows reference host)

| Option | Offline / no clang | Portable sandbox | Ownership/Drop depth | Maintenance surface |
| --- | --- | --- | --- | --- |
| Packaged native toolchain (`sgc`/`sgpm` archives) | Yes after install | Host OS | Full native production semantics | Already required for release |
| WASM scalar backend (`--target wasm`) | Build needs no clang; run needs Node/wasmtime | Yes | Scalar + fail-closed unsupported owned types | One emitter + validator |
| Scalar `SGB1` prototype | Yes (`PATH` stripped e2e) | Interpreter only | Scalar only; no heap/Drop | Second IR + verifier + host ABI |

Local evidence commands:

```text
cargo test -p sgc --test portable_targets bytecode_target_builds_and_runs_without_native_toolchain
cargo test -p sgc --test portable_targets wasm_target_emits_a_valid_exported_main_module
cargo test -p sgc --test portable_targets wasm_target_runs_scalar_main_with_host_runtime
```

Artifact size / startup observations for the scalar `choose(40)` fixture:

- `.sgbc` and `.wasm` are both small pure-IR artifacts; neither approaches the
  product need of shipping ownership-correct generic collections or stdlib I/O.
- Clang-free bytecode run works, but so does packaged native install, which
  already solves “no local toolchain” for real applications.

## Rationale for NO-GO

1. **User need covered:** installable native archives plus WASM portable deploy
   cover the two dominant stories (real apps; sandboxed scalar/portable).
2. **Second runtime cost:** a production VM must re-implement ownership, Drop,
   verifier threat model, host-call allowlists, and differential conformance—
   roughly a third backend after LLVM-native and WASM.
3. **Prototype is not evidence of product value:** `SGB1` version 1 only proves
   scalar control flow; promoting it would freeze a weak format or force a
   breaking rewrite immediately.

## Decision

- **Cancel** production implementation of `bytecode-vm-v1` tasks 2+.
- Keep the experimental `sgc build/run --target bytecode` path as a **non-
  supported research prototype** (capability matrix: experimental).
- Prefer investing in WASM WASI subset growth and native production quality.

This document is the replacement OpenSpec cancellation evidence required by
the bytecode child design and the coordinator go/no-go requirement.
