## Why

WASM and a portable bytecode VM are both valuable long-range deployment
options, but they are not one implementation surface. WASM targets a sandboxed
external platform and WASI host contract; bytecode introduces a second Sengoo
runtime, artifact verifier, and interpreter. Keeping both under one
implementation owner would create ambiguous tasks and archive criteria.

They also depend on a versioned native MIR/runtime semantic checkpoint. Starting
before generic collections, concurrency, distribution, and production
hardening stabilize would duplicate moving ownership and host-ABI rules.

## Proposal

Convert this change into the post-v1 backend coordinator:

- enforce the stable-ABI and roadmap entry review;
- define the backend-neutral MIR semantic version and portable runtime ABI
  artifact consumed by both children;
- track independent `wasm-backend-v1` and `bytecode-vm-v1` child changes;
- keep one cross-target capability matrix and differential conformance policy;
- allow the bytecode go/no-go review to cancel the VM if evidence does not
  justify a second runtime.

Backend implementation requirements are owned only by the child changes. The
coordinator owns only the shared entry contract and cross-target policy.

## Impact

- Parent: `language-maturity-roadmap`, post-v1 phase.
- Children: `wasm-backend-v1`, `bytecode-vm-v1`.
- Shared entry work touches the compiler MIR API, `sgc` frontend bundle, the
  portable ABI artifact, diagnostics, and contract tests. Emitter, VM, WASI,
  and interpreter implementation remains in the child changes.

## Non-goals

- Implementing either backend in this coordinator.
- Blocking the earlier mainstream-default release.
- Treating Cranelift parity as WASM or bytecode work.
