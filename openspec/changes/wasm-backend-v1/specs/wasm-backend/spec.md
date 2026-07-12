## ADDED Requirements

### Requirement: The compiler SHALL produce validated WebAssembly modules

After the backend entry gate passes, `sgc build --target wasm` SHALL produce a
valid module using the selected, documented emitter and target ABI.

#### Scenario: Core program is built for WASM

- **WHEN** a representative scalar/control-flow/call/aggregate program is built
- **THEN** module validation succeeds
- **AND** execution under the pinned runtime matches native expected output and
  exit behavior

### Requirement: WASM SHALL preserve ownership and Drop semantics

Owned values in linear memory SHALL follow the same move and exact-once Drop
contract as native production semantics.

#### Scenario: Owned aggregate exits through multiple paths

- **WHEN** a program containing String, Vec, or user Drop values exits normally,
  early, or through propagated error
- **THEN** every still-owned value is released exactly once
- **AND** no linear-memory access uses a moved or freed value

### Requirement: WASI host capabilities SHALL be versioned and bounded

The backend SHALL publish a pinned WASI profile and supported import subset with
resource limits.

#### Scenario: Program uses a supported WASI capability

- **WHEN** a program uses documented args/env/output/time/file APIs
- **THEN** imports use the versioned host ABI and execute within configured
  memory/time/output limits

### Requirement: Unsupported WASM capabilities SHALL fail explicitly

Host-only or unsupported stdlib capabilities SHALL fail before emission with a
stable target diagnostic and SHALL NOT fall back to native execution.

#### Scenario: WASM program imports dynamic FFI or process APIs

- **WHEN** the selected target does not support the import
- **THEN** build fails with `unsupported-target-capability`
- **AND** no native artifact or subprocess is produced
