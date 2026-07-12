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

## Entry gate

- `mainline-release-baseline` archived;
- numeric, generic collections, debugger, registry/distribution, concurrency,
  and production hardening archive gates pass;
- native MIR/runtime ABI is versioned;
- a released toolchain and conformance corpus are available;
- each backend has a user story, owner, support tier, and resource budget.

## Backend-specific direction

- WASM targets WASI first; browser/component-model work is separate.
- Bytecode defines ownership/Drop, validation, resource limits, and portable
  serialization before interpreter optimization.
- Full Cranelift parity remains separate from both.
