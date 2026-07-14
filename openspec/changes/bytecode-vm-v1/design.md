## Context

The VM must reproduce native language semantics while treating bytecode as
hostile input. Its value is unproven until measured against packaged native and
WASM alternatives.

The repository already contains a scalar `SGB1` emitter/interpreter. It is a
disposable prototype, not a stable format. Its current version byte does not
pre-empt the value review, threat model, verifier design, or compatibility
decision in this change.

## Entry and cancellation gate

Implementation starts only after coordinator tasks 1.2-1.6 pass and a
documented go decision. The VM consumes the versioned portable runtime ABI and
never embeds native runtime addresses. The change may be replaced by a
cancellation decision without implementing tasks 2+.

## Decisions

### Decision 1: Typed register bytecode

The format is a typed register IR lowered from validated MIR. It contains:

- magic, format version, runtime ABI version, flags, and checksum;
- type/layout table, constants, functions, blocks, and exception/error metadata;
- explicit move, borrow/load/store, call, branch, aggregate, and drop
  instructions;
- versioned host-call identifiers rather than raw native addresses.

### Decision 2: Verify before execution

The verifier checks bounds, register/type consistency, control-flow targets,
definite initialization, move/use rules, call signatures, resource limits, and
required Drop paths. Invalid bytecode cannot reach the interpreter loop.

### Decision 3: Ownership is explicit in VM state

Registers/slots track initialized, moved, borrowed where required, and dropped
state consistent with source semantics. Scope/early return/error cleanup uses
compiler-emitted drop plans verified before execution.

### Decision 4: Host calls are allowlisted and bounded

The first VM host ABI supports only capabilities needed by the conformance and
CLI fixture. Calls validate pointer/length/handle equivalents, enforce resource
limits, and return stable status values. Dynamic native FFI is unsupported.

### Decision 5: No-clang means no hidden native compilation

Bytecode build and run work with clang unavailable. Native fallback is a hard
test failure.

## Value metrics

- cold startup versus packaged native `sgc run`;
- bytecode artifact size;
- interpreter throughput on CLI-scale workloads;
- implementation/maintenance surface and host portability.

If value is not material, record cancellation rather than preserving a weak
second runtime.

## Archive gate

- versioned format and verifier threat model;
- malformed-bytecode corpus cannot panic or escape limits;
- core differential conformance and ownership/Drop tests pass;
- clang-free CLI scenario passes on Windows, Linux, and macOS;
- value metrics justify continuation and are documented.
