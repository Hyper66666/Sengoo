## ADDED Requirements

### Requirement: The toolchain SHALL build typed python bindings and installable wheels

The Python build backend SHALL implement the following typed binding and self-contained distribution requirements.

#### Scenario: Stage deliverables are reviewed

- **WHEN** this stage is considered for completion
- **THEN** the following implementation and evidence obligations are satisfied:

- Generate typed bridge metadata from checked nongeneric synchronous function types.
- Implement scalar/string/numeric-vector copies, owned cleanup and Result error conversion.
- Implement sengoo-python PEP 517 backend, ctypes PyDLL wrappers, stubs and platform wheels.
- Validate module ABI isolation, unsupported signatures, bounds and failure cleanup.
- Add Python batch consumer and four-host CPython 3.11 through 3.14 wheel jobs.

#### Scenario: A Python user installs a prebuilt wheel in a clean environment

- **WHEN** a Python user installs a prebuilt wheel in a clean environment
- **THEN** the module imports and executes without Sengoo, Rust or clang.
- **AND** existing compatible v0.2 projects SHALL retain their documented behavior.
