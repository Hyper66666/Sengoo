## Why

A portable bytecode VM could provide clang-free execution, fast startup, and a
controlled sandbox for scripts. It also creates a second runtime and long-term
semantic burden, so it proceeds only after a measured go/no-go review.

## Proposal

- Define a versioned, validated, typed register bytecode derived from MIR.
- Implement an interpreter with ownership/Drop, bounded resources, and a
  versioned host-call allowlist.
- Add `sgc build/run --target bytecode` without invoking clang.
- Differentially run the core conformance corpus against native semantics.
- Measure startup, artifact size, portability, and maintenance value; permit
  cancellation through the coordinator if the VM is not justified.

## Impact

- Bytecode format/verifier/serializer, MIR lowering, interpreter/runtime host
  calls, sgc target selection, tests, docs, and capability matrix.
- Parent coordinator: `wasm-and-bytecode-backends`.
- Begins only after its go/no-go and stable-ABI entry gates pass.

## Non-goals

- JIT compilation, native extension ABI, or bytecode package distribution in v1.
- WASM implementation.
- Running unvalidated or ABI-incompatible bytecode.
