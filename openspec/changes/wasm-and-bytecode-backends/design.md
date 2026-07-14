## Context

Alternative backends are valuable, but today the native MIR/runtime ABI, generic
collection representation, release process, and concurrency surface are still
moving. Implementing WASM and a bytecode VM now would duplicate unstable
semantics and multiply the test matrix before the default path is adopted.

## Decisions

### Decision 1: This change is post-v1 and does not block mainstream-default

The change remains in the long-range program, but implementation waits for the
entry gate below. Documentation/capability-matrix corrections may continue.

### Decision 2: Split WASM and bytecode before implementation

WASM/WASI deployment and a portable interpreter have different users,
architectures, and runtime contracts. Before code begins, create independent
owner changes (`wasm-backend-v1` and `bytecode-vm-v1`) and let this change become
their coordinating design/history record.

### Decision 3: Native semantics are the differential oracle

Alternative backends consume versioned MIR/runtime semantics and run the same
core conformance corpus. Unsupported stdlib capabilities fail with stable target
diagnostics; they never silently fall back to host-native execution.

### Decision 4: A go/no-go review may cancel the bytecode VM

The bytecode VM proceeds only if measured install/startup/portability value
justifies a second runtime. A recorded cancellation is acceptable if WASM,
packaged clang, or Cranelift solves the user need with lower maintenance cost.

### Decision 5: MIR v1 is an in-process semantic contract

The stable MIR checkpoint versions the meaning of types, control flow, calls,
ownership transitions, Drop plans, target pointer width, dynamic-dispatch slot
ordinals, and async lifecycle operations consumed by backends. It is not a
serialized MIR file format and does not promise that Rust struct layout or
debug formatting is stable.

Every backend compilation request carries an explicit target pointer width and
MIR semantic ABI version. Host pointer width is only the default for native
compilation. WASM v1 always requests 32-bit pointer-sized lowering.

### Decision 6: Portable runtime ABI is independent of the native C ABI

`runtime/abi/portable_runtime_abi_v1.json` is the canonical, machine-readable
portable runtime contract. It records logical layout identifiers, field widths,
ownership transitions, Drop and dyn-vtable slot ordinals, async lifecycle
operations, versioned host-call identifiers, and resource-limit categories.

The portable contract cannot expose native addresses or C-layout vocabulary
such as `void*`, `size_t`, function pointers, or platform handles. The existing
`tools/stdlib/runtime_shared.h` remains the native C ABI and is not imported as
the WASM or bytecode ABI.

### Decision 7: Existing portable outputs are experimental prototypes

The current scalar direct-WASM emitter and `SGB1` interpreter are evidence
spikes. Their artifact bytes, opcode numbers, and behavior are not compatibility
promises. Each child change must explicitly promote, replace, or discard its
prototype after the entry review. In particular, current `SGB1` version 1 does
not become a stable format merely because it contains a version field.

### Decision 8: Unsupported target behavior has a stable diagnostic code

All target capability rejection uses `unsupported-target-capability` as the
stable diagnostic code and identifies the target plus capability. Human text
may evolve, but portable builds never silently lower unsupported operations to
scalar moves and never fall back to native execution.

## Entry gate

- `mainline-release-baseline` archived;
- numeric, generic collections, debugger, registry/distribution, concurrency,
  and production hardening archive gates pass;
- native MIR/runtime ABI is versioned;
- a released toolchain and conformance corpus are available;
- each backend has a user story, owner, support tier, and resource budget.

The ABI portion of the gate is complete only when:

- compiler and `sgc` expose target-aware MIR compilation without consulting
  host pointer width;
- a MIR semantic ABI version is carried with the resulting bundle and unknown
  versions are rejected before backend lowering;
- the portable runtime ABI artifact passes schema, version, ordinal, and
  forbidden-native-vocabulary checks;
- wasm32 pointer-sized literal/type lowering has a regression test;
- unsupported operations produce `unsupported-target-capability`; and
- both child designs classify the current prototypes and consume this shared
  contract rather than private ABI assumptions.

## Backend-specific direction

- WASM targets WASI first; browser/component-model work is separate.
- Bytecode defines ownership/Drop, validation, resource limits, and portable
  serialization before interpreter optimization.
- Full Cranelift parity remains separate from both.
