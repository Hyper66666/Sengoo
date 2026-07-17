# backend-targets Specification

## Purpose
Coordinate post-v1 alternative backends: versioned MIR/runtime ABI entry gate, independent WASM and bytecode owners, differential native oracle policy, and bytecode go/no-go.

## Requirements

### Requirement: Alternative backend work SHALL pass a stable-ABI entry gate

WASM and bytecode implementation SHALL begin only after the native MIR/runtime
ABI is versioned and the roadmap's default-library, distribution, concurrency,
and production-hardening gates pass.

#### Scenario: A child backend is proposed before the entry gate

- **WHEN** one or more prerequisite gates or ABI versions are missing
- **THEN** implementation remains deferred
- **AND** design/capability-matrix corrections may continue without claiming
  backend support

### Requirement: Portable backends SHALL consume versioned target-aware MIR

The compiler SHALL expose an in-process MIR semantic contract with an explicit
ABI version and target pointer width. Portable compilation SHALL NOT infer
pointer-sized language semantics from the build host.

#### Scenario: MIR is compiled for wasm32 on a 64-bit host

- **WHEN** the frontend compiles a program containing `usize`, `isize`, or
  pointer-sized literals for the WASM target
- **THEN** parsing, type checking, and MIR lowering use 32-bit semantics
- **AND** the resulting MIR bundle records the supported MIR semantic ABI
  version and 32-bit target width

#### Scenario: A backend receives an unknown MIR version

- **WHEN** a backend is asked to lower a MIR bundle with an unsupported semantic
  ABI version
- **THEN** it rejects the bundle before emission or execution
- **AND** it does not reinterpret the bundle using the current implementation

### Requirement: Portable runtime semantics SHALL have a canonical ABI artifact

The project SHALL publish a versioned machine-readable portable runtime ABI
that models logical layouts, ownership transitions, Drop/dyn slot ordinals,
async lifecycle operations, host-call identifiers, and resource limits without
embedding native addresses or platform C types.

#### Scenario: The portable ABI contract is validated

- **WHEN** contract tests load the canonical portable runtime ABI artifact
- **THEN** its schema, version, required identifiers, and ordinals are valid
- **AND** native pointer vocabulary including `void*`, `size_t`, raw function
  pointers, and platform handles is rejected

### Requirement: Existing portable artifacts SHALL remain experimental until promoted

The current scalar WASM and `SGB1` implementations SHALL be treated as
experimental prototypes. Child changes SHALL explicitly promote, replace, or
discard them and SHALL NOT infer compatibility from their current magic or
version bytes.

#### Scenario: A prototype artifact is encountered

- **WHEN** an artifact was produced before its child change passes the archive
  gate
- **THEN** the toolchain does not promise forward compatibility
- **AND** documentation identifies the target as experimental

### Requirement: WASM and bytecode SHALL have independent owners

WASM implementation SHALL be owned by `wasm-backend-v1` and bytecode
implementation SHALL be owned by `bytecode-vm-v1`. This coordinator SHALL NOT
own their compiler or runtime implementation tasks.

#### Scenario: Backend implementation begins

- **WHEN** code implementation starts for WASM or bytecode
- **THEN** the corresponding child change has passed strict validation
- **AND** its design, tests, migration, and archive gate are independently
  reviewable

### Requirement: Alternative targets SHALL share differential conformance policy

The child backends SHALL use native production semantics as the differential
oracle and SHALL publish one capability matrix with stable unsupported-target
diagnostics.

#### Scenario: A target cannot support a stdlib capability

- **WHEN** a program uses a capability absent from the selected target
- **THEN** build fails with diagnostic code `unsupported-target-capability`
- **AND** the diagnostic identifies the selected target and missing capability
- **AND** it does not silently execute through the native backend

### Requirement: The bytecode VM SHALL pass a go/no-go value review

The bytecode VM SHALL proceed only after a positive value review. It may be
cancelled by an explicit replacement decision when measured startup,
portability, or tooling value does not justify a second runtime.

#### Scenario: The VM is not justified

- **WHEN** the entry review finds a packaged native toolchain, WASM, or
  experimental JIT satisfies the user need with lower maintenance cost
- **THEN** a replacement OpenSpec may cancel `bytecode-vm-v1`
- **AND** native and WASM support claims remain unchanged

