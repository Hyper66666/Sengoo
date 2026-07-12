## ADDED Requirements

### Requirement: Bytecode SHALL be versioned and verified before execution

The VM SHALL accept only bytecode with supported format/runtime ABI versions and
valid types, control flow, initialization, move, call, Drop, and resource-limit
metadata.

#### Scenario: Malformed bytecode is loaded

- **WHEN** bytecode is truncated, mutated, out of bounds, type-inconsistent, or
  has an invalid move/Drop plan
- **THEN** verification rejects it before interpretation
- **AND** rejection cannot panic, allocate beyond configured limits, or invoke a
  host call

### Requirement: VM execution SHALL preserve ownership and Drop semantics

The interpreter SHALL implement the native move, borrow, initialization, and
exact-once Drop contract for supported programs.

#### Scenario: VM program moves and removes owned values

- **WHEN** a program moves String or generic collection values through calls and
  exits through normal/error paths
- **THEN** moved values cannot be reused
- **AND** each still-owned value is dropped exactly once
- **AND** differential results match native semantics

### Requirement: VM execution SHALL be clang-free and bounded

`sgc run --target bytecode` SHALL build and execute without invoking clang or a
native backend and SHALL enforce configured instruction/time/memory/output
limits.

#### Scenario: VM runs where clang is unavailable

- **WHEN** a representative program is built and run with clang removed from
  the environment
- **THEN** it produces the expected output through the interpreter
- **AND** exceeding a resource limit terminates with a stable VM status

### Requirement: VM host calls SHALL be versioned and allowlisted

Bytecode SHALL reference versioned host-call identifiers and SHALL NOT contain
or execute arbitrary native addresses.

#### Scenario: Bytecode requests an unsupported host call

- **WHEN** a verified program references a host capability outside the target
  matrix
- **THEN** verification or execution fails with a stable unsupported-capability
  status
- **AND** no dynamic native FFI fallback occurs
