## Context

Sengoo MIR and runtime currently assume native pointers, C/native runtime
objects, and platform handles in several stdlib areas. WASM requires a bounded
linear-memory ABI and a documented host capability subset.

The repository already contains a scalar direct emitter. It is a disposable
prototype: this change must promote, replace, or discard it after comparing it
with the LLVM-target path. Existing `.wasm` bytes are not a compatibility
contract.

## Entry gate

Implementation starts only after `wasm-and-bytecode-backends` tasks 1.2-1.6
pass. The child consumes MIR semantic ABI v1 and the canonical portable runtime
ABI; it does not consume native C pointers from `runtime_shared.h`.

## Decisions

### Decision 1: Select emitter through an evidence spike

Prototype representative scalar, control-flow, aggregate, call, Drop, and
stdlib-import programs through:

- the existing LLVM-text path targeting WASM; and
- a minimal direct MIR-to-WASM emitter if LLVM integration cannot preserve the
  required ABI or packaging path.

Record compile time, artifact validity, diagnostics, runtime integration, and
maintenance cost. Choose one production path; do not maintain two.

### Decision 2: Target WASI first

The first target is a pinned WASI runtime/profile recorded at implementation
time. Supported imports begin with args/env, stdout/stderr, monotonic time, and
sandbox-permitted file IO. Network/process/dynamic FFI support is absent unless
separately specified and tested.

### Decision 3: Preserve ownership and Drop in linear memory

MIR allocation, aggregate layout, String/Vec descriptors, trait-object
vtables, and drop glue use 32-bit linear-memory offsets under the target ABI.
Each owned value drops exactly once. Traps and early returns run required cleanup
unless the runtime terminates the whole instance.

WASM v1 always requests `TargetPointerWidth::Bits32`. A host-width MIR bundle is
an input error, not something the emitter truncates during code generation.

### Decision 4: Validate modules and bound resources

Generated modules pass a standard validator before being reported successful.
Runtime invocation has documented memory/fuel/time limits for tests. Host import
names and ABI version are explicit.

### Decision 5: Unsupported capabilities are compile errors

Target capability analysis happens before emission and reports stable
`unsupported-target-capability` diagnostics. No native subprocess or hidden
fallback is used.

## Archive gate

- chosen emitter decision recorded;
- representative core conformance passes under the pinned WASM runtime;
- ownership/Drop and trap/error scenarios pass;
- supported WASI stdlib subset and negative diagnostics documented;
- CI produces and runs `.wasm` artifacts on at least two host OS families.
