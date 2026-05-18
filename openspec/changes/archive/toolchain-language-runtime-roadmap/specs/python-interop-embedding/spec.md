## ADDED Requirements

### Requirement: Runtime SHALL provide robust CPython embedding support
`runtime/src/python.rs` integration SHALL support stable CPython embedding for invoking Python from Sengoo programs.

#### Scenario: Sengoo code invokes embedded Python API
- **WHEN** a Sengoo program uses supported Python interop calls
- **THEN** embedded CPython execution succeeds with proper error propagation on failure

### Requirement: Sengoo modules SHALL be exportable as Python extension modules
The toolchain SHALL support building Sengoo modules as Python-compatible extension artifacts.

#### Scenario: Built artifact can be imported by Python
- **WHEN** a Sengoo module is compiled for Python extension output
- **THEN** Python can import the artifact and call exported interfaces
