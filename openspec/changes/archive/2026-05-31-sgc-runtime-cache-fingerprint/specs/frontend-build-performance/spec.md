## ADDED Requirements

### Requirement: Native runtime cache reuse SHALL depend on runtime source bytes
`sgc run`, `sgc build`, and the native runtime object cache SHALL treat the
current runtime C source bytes as part of native linkage identity.

#### Scenario: Runtime bytes change at the same path
- **WHEN** a cached native artifact records a runtime C path and fingerprint
- **AND** the current runtime C source has the same path but different bytes
- **THEN** `sgc` treats the artifact metadata as a cache miss
- **AND** reports that the runtime source changed
- **AND** relinks with an object compiled from current runtime bytes

#### Scenario: Runtime object bytes change without a length change
- **WHEN** runtime C source bytes change while the canonical path, byte length,
  optimization level, and target remain unchanged
- **THEN** runtime object-cache identity changes
- **AND** `sgc` does not reuse the object compiled from the previous bytes

#### Scenario: Runtime bytes stay unchanged
- **WHEN** runtime C source bytes, path, optimization level, and target remain
  unchanged
- **THEN** runtime object-cache identity remains stable
- **AND** otherwise matching run/build metadata can reuse cached artifacts

#### Scenario: Older metadata lacks a runtime fingerprint
- **WHEN** older metadata deserializes without a runtime fingerprint
- **AND** the current native linkage uses runtime C source with a fingerprint
- **THEN** `sgc` treats the metadata as stale
- **AND** performs a one-time rebuild instead of reusing an unverifiable native
  artifact
