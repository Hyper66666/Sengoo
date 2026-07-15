# wasm-backend Specification

## Purpose

Define the experimental scalar wasm32 backend for Sengoo: a direct MIR-to-WASM
emitter with validated modules, versioned ABI metadata, and fail-closed
capability diagnostics. Production ownership, Drop, and WASI host coverage are
explicitly out of scope for this specification.

## Requirements

### Requirement: Experimental scalar WASM SHALL produce validated modules

The toolchain SHALL produce a core WebAssembly module for experimental scalar
programs when building with target wasm, and SHALL reject aggregates, heap
ownership, FFI, and unsupported host imports with unsupported-target-capability.

#### Scenario: Scalar program is built for WASM

- **WHEN** a scalar control-flow or call program is built with target wasm
- **THEN** module validation succeeds
- **AND** the module exports main with type () -> i64
- **AND** embedded MIR semantic ABI and portable runtime ABI versions are present

#### Scenario: Parameterized main is rejected

- **WHEN** a program defines main with one or more parameters
- **THEN** build fails with unsupported-target-capability
- **AND** external wasm artifacts whose exported main is not () -> i64 fail
  validation before host execution

#### Scenario: Aggregate or host-only program is built for WASM

- **WHEN** a program requires aggregates, heap ownership, FFI, or unsupported
  stdlib or host imports
- **THEN** build fails with unsupported-target-capability
- **AND** no native fallback artifact is produced

### Requirement: WASM integer operations SHALL preserve signedness

Division, remainder, shift, and ordered comparison operations SHALL use
unsigned WebAssembly opcodes when operands are unsigned integer types.

#### Scenario: Unsigned compare of maximum u64 and zero

- **WHEN** a program evaluates maximum u64 greater than zero on the WASM target
- **THEN** the result matches native production semantics

### Requirement: WASM artifacts SHALL reject unknown ABI versions before run

Running a wasm artifact SHALL parse the portable ABI custom section and SHALL
reject unsupported MIR or portable runtime ABI versions before invoking a host
runtime.

#### Scenario: Tampered ABI version is executed

- **WHEN** an otherwise valid scalar module has its embedded ABI version changed
  to an unsupported value
- **THEN** run fails with unsupported-mir-semantic-abi or
  unsupported-portable-runtime-abi
- **AND** the host runtime is not used to execute the module body

### Requirement: Unsupported memory operations SHALL fail closed

Load, Store, and AddrOf operations SHALL fail with unsupported-target-capability
and MUST NOT be rewritten to a plain Move. Ref, Ptr, and Future types are
outside the experimental scalar surface.

#### Scenario: Program uses AddrOf or Load

- **WHEN** portable lowering encounters AddrOf, Load, or Store
- **THEN** compilation fails with a stable capability diagnostic
- **AND** the instruction is not rewritten to a plain Move

### Requirement: Production ownership Drop and WASI MUST remain deferred

The experimental scalar backend MUST NOT claim production ownership Drop or WASI
host support until a follow-up change implements and archives those surfaces.

#### Scenario: Documentation describes the experimental boundary

- **WHEN** users read portable-targets or wasm-wasi-profile documentation
- **THEN** the experimental scalar tier and deferred WASI ownership work are
  stated explicitly
