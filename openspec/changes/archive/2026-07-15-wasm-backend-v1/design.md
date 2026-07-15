## Context

Sengoo MIR and runtime currently assume native pointers, C/native runtime
objects, and platform handles in several stdlib areas. The implemented WASM
path is narrower: it emits pure core WebAssembly for a scalar MIR subset, keeps
the ABI boundary explicit, and rejects unsupported ownership and host features
instead of emulating them.

The repository already contains a scalar direct emitter. The comparison against
the LLVM-target path is complete, and this change now records the experimental
scalar contract that matches the shipped implementation. Existing `.wasm` bytes
are not a broader compatibility contract.

## Entry gate

Implementation starts only after `wasm-and-bytecode-backends` tasks 1.2-1.6
pass. The child consumes MIR semantic ABI v1 and the canonical portable runtime
ABI; it does not consume native C pointers from `runtime_shared.h`.

## Decisions

### Decision 1: Keep the direct emitter as the experimental scalar path

Prototype representative scalar, control-flow, aggregate, call, Drop, and
stdlib-import programs through:

- the existing LLVM-text path targeting WASM; and
- a minimal direct MIR-to-WASM emitter if LLVM integration cannot preserve the
  required ABI or packaging path.

The evidence spike is complete. The direct MIR-to-WASM emitter remains the v1
path because it ships inside `sgc`, produces validated core modules, and avoids
host `clang` / WASI SDK packaging requirements. LLVM-to-WASM is not part of
this experimental scalar change.

### Decision 2: Keep v1 as pure core WebAssembly and defer WASI

The first supported surface is `wasm32-unknown-unknown` with no required host
imports. `docs/wasm-wasi-profile.md` may record a future WASI allowlist, but
documenting candidate imports is not evidence that args/env/stdout/stderr/time
or file IO are implemented. Any WASI host profile remains follow-up work.

### Decision 3: Fail closed at the ownership and memory boundary

Experimental v1 supports scalar values, control flow, and internal calls only.
Aggregates, heap ownership/Drop, String/Vec descriptors, trait-object vtables,
`Load`, `Store`, `AddrOf`, and Ref/Ptr/Future surfaces remain compile-time
`unsupported-target-capability` diagnostics. The backend must fail closed and
must not rewrite unsupported memory instructions into plain `Move`s or fall
back to native execution.

WASM v1 always requests `TargetPointerWidth::Bits32`. A host-width MIR bundle is
an input error, not something the emitter truncates during code generation.

### Decision 4: Preserve wasm32 ABI metadata and unsigned semantics

Generated modules export `main`, embed MIR semantic ABI v1 plus portable
runtime ABI v1 metadata, and use unsigned WebAssembly opcodes for unsigned
division, remainder, shifts, and ordered comparisons. `sgc run --target wasm`
must reject unsupported embedded ABI versions before invoking a host runtime.

### Decision 5: Keep hardening claims limited to implemented guardrails

Generated modules pass a standard validator before being reported successful.
Current enforced guardrails are limited to module-size validation, embedded ABI
validation, and wall-clock timeout around Node or wasmtime execution. Runtime
memory ceilings, output limits, and completed Windows plus Unix CI execution
coverage remain follow-up work and must not be claimed complete from
documentation alone.

## Archive gate (experimental scalar)

This change archives when:

- experimental scalar emitter, validation, `main: () -> i64`, signedness, and
  fail-closed unsupported surfaces are implemented and tested;
- docs and SUPPORT_MATRIX state Experimental / deferred (not production
  Supported for portable targets); and
- deferred WASI / ownership-Drop / multi-OS CI items are recorded as successor
  work rather than silently claimed complete.

## Successor work (not this archive)

- WASI host import subset with runtime tests;
- ownership/Drop and aggregate/heap lowering;
- runtime memory/output ceilings beyond module size + wall-clock timeout;
- Windows + Unix CI execution of `.wasm` artifacts.
