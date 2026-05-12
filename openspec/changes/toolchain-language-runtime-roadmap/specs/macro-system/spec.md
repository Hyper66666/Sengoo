## ADDED Requirements

### Requirement: Sengoo SHALL support declarative macros
The language SHALL provide declarative macro definitions and expansion rules similar to pattern-based macro systems.

#### Scenario: Declarative macro expands into valid source form
- **WHEN** a valid declarative macro invocation is compiled
- **THEN** macro expansion produces valid Sengoo syntax before type checking

### Requirement: Sengoo SHALL support procedural derive macros
The toolchain SHALL support procedural macro hooks for custom derive-style code generation.

#### Scenario: Custom derive generates required implementation
- **WHEN** a type uses a supported procedural derive macro
- **THEN** generated implementation items are available to later compilation phases
