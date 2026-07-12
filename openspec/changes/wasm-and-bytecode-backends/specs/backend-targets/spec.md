## ADDED Requirements

### Requirement: Alternative backend work SHALL pass a stable-ABI entry gate

WASM and bytecode implementation SHALL begin only after the native MIR/runtime
ABI is versioned and the roadmap's default-library, distribution, concurrency,
and production-hardening gates pass.

#### Scenario: A child backend is proposed before the entry gate

- **WHEN** one or more prerequisite gates or ABI versions are missing
- **THEN** implementation remains deferred
- **AND** design/capability-matrix corrections may continue without claiming
  backend support

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
- **THEN** build fails with a stable target/capability diagnostic
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
