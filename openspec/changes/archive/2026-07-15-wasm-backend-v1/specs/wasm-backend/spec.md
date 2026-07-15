## MODIFIED Requirements

### Requirement: Experimental scalar WASM SHALL produce validated modules

`sgc build --target wasm` SHALL produce a core WebAssembly module for programs
that lower to the experimental scalar MIR subset, using the documented direct
emitter and wasm32 frontend semantics.

#### Scenario: Scalar program is built for WASM

- **WHEN** a scalar control-flow/call program is built with `--target wasm`
- **THEN** module validation succeeds
- **AND** the module exports `main`
- **AND** embedded MIR semantic ABI and portable runtime ABI versions are present
  and supported

#### Scenario: Aggregate or host-only program is built for WASM

- **WHEN** a program requires aggregates, heap ownership, FFI, or unsupported
  stdlib/host imports
- **THEN** build fails with `unsupported-target-capability`
- **AND** no native fallback artifact is produced

### Requirement: WASM integer ops SHALL preserve signedness

Division, remainder, shifts, and ordered comparisons SHALL use unsigned WASM
opcodes when operands are unsigned integer types.

#### Scenario: Unsigned compare of u64::MAX and zero

- **WHEN** a program evaluates `18446744073709551615u64 > 0u64` on the WASM target
- **THEN** the result matches native production semantics

### Requirement: WASM artifacts SHALL reject unknown ABI versions before run

`sgc run --target wasm` on a `.wasm` artifact SHALL parse the portable ABI
custom section and reject unsupported MIR or portable runtime ABI versions
before invoking a host runtime.

#### Scenario: Tampered ABI version is executed

- **WHEN** an otherwise valid scalar module has its embedded ABI version changed
  to an unsupported value
- **THEN** run fails with `unsupported-mir-semantic-abi` or
  `unsupported-portable-runtime-abi`
- **AND** the host runtime is not used to execute the module body

### Requirement: Unsupported memory operations SHALL not be silently rewritten

Load, Store, and AddrOf SHALL fail with `unsupported-target-capability`.
Ref/Ptr/Future types are outside the experimental scalar surface.

#### Scenario: Program uses AddrOf or Load

- **WHEN** portable lowering encounters AddrOf, Load, or Store
- **THEN** compilation fails with a stable capability diagnostic
- **AND** the instruction is not rewritten to a plain Move

### Requirement: Production ownership/Drop and WASI MUST remain deferred

This experimental scalar backend MUST NOT claim production ownership/Drop or
WASI host support until a follow-up change implements them.

#### Scenario: Documentation describes the experimental boundary

- **WHEN** users read `docs/portable-targets.md` or `docs/wasm-wasi-profile.md`
- **THEN** the experimental scalar tier and deferred WASI/ownership work are
  stated explicitly
