## ADDED Requirements

### Requirement: Large binary Base64 encoding requires an explicit bound

The standard library SHALL provide an additive borrowed-buffer Base64 helper
with an explicit maximum from 1 through 16777216 bytes. The helper SHALL read
only initialized input bytes and SHALL write only within caller-owned output
capacity. It SHALL retain no source, destination or scratch resources.

#### Scenario: Input exceeds the legacy limit within the explicit maximum

- **WHEN** a caller supplies more than 65536 initialized bytes within its explicit maximum
- **THEN** the helper SHALL return ordinary RFC4648 Base64 for the complete range
- **AND** only the final encoded group SHALL contain padding
- **AND** embedded NUL SHALL remain an exact binary byte

#### Scenario: A caller supplies an invalid or insufficient bound

- **WHEN** input exceeds its explicit maximum or output capacity is insufficient
- **THEN** encoding SHALL fail with overflow status 13 before conversion
- **AND** an invalid maximum, negative/uninitialized length or aliased buffers SHALL fail with status 2
- **AND** callers SHALL discard any partial output if a later conversion fails

#### Scenario: A caller supplies empty input

- **WHEN** initialized input length and output capacity are both zero with a valid maximum
- **THEN** encoding SHALL succeed with zero output without dereferencing input

### Requirement: Existing encoding boundaries remain compatible

Legacy Base64 functions SHALL retain their symbols, signatures, 65536-byte
input maximum and prior failure behavior without a runtime ABI change.

#### Scenario: An existing caller uses the former boundary

- **WHEN** an existing caller encodes exactly 65536 bytes
- **THEN** its prior successful behavior SHALL remain
- **AND** 65537 bytes SHALL continue to return its existing status 2
