## ADDED Requirements

### Requirement: An authoritative, versioned language reference SHALL match the implementation

The project SHALL maintain a single language reference that documents the
implemented language, with a per-construct status linked to proof, and SHALL
mark the legacy design draft historical.

#### Scenario: Reference claim matches the compiler

- **WHEN** the reference documents a language construct as Supported
- **THEN** a linked example or test demonstrates that construct compiling/running
- **AND** constructs that are not implemented are marked unsupported or removed,
  not presented as available

#### Scenario: Legacy draft is redirected

- **WHEN** a reader opens `Sengoo_Language_Specification.md`
- **THEN** it is marked historical and points to the authoritative reference

### Requirement: Reference examples SHALL be verified by CI doc-tests

Code blocks in the reference SHALL be compiled (and run where applicable) by CI
so the reference cannot drift from the compiler.

#### Scenario: A drifting reference example fails CI

- **WHEN** a reference code block no longer compiles or run-produces its
  documented result
- **THEN** the doc-test CI job fails
- **AND** the failure identifies the offending reference section

### Requirement: The reference SHALL be versioned with the toolchain

The reference SHALL declare which toolchain version it describes and follow a
documented versioning policy.

#### Scenario: Reference declares its version

- **WHEN** a user reads the reference
- **THEN** it states the toolchain version it corresponds to
- **AND** the versioning policy for updates is documented
