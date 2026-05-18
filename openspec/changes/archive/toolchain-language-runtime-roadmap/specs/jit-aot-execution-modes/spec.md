## ADDED Requirements

### Requirement: Toolchain SHALL support fast JIT mode for development
The runtime/toolchain SHALL provide a Cranelift-backed fast JIT execution mode for iterative development.

#### Scenario: Developer run uses fast JIT mode
- **WHEN** a developer runs Sengoo with JIT mode enabled
- **THEN** code is compiled and executed through the JIT path with low startup latency

### Requirement: Toolchain SHALL support AOT output for production
The toolchain SHALL provide an ahead-of-time compilation mode that produces deployable production artifacts.

#### Scenario: Production build emits AOT artifact
- **WHEN** a project is built in production AOT mode
- **THEN** the toolchain emits a runnable artifact without requiring JIT at runtime
