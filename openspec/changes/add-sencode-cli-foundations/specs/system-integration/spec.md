## ADDED Requirements

### Requirement: New modules are distributed and source compatible
All new modules SHALL be discoverable by sgc/sglsp and included in installed
toolchains while edition 2026 and runtime ABI 1 remain unchanged.

#### Scenario: Existing project builds
- **WHEN** a retained v0.2 fixture is built with the new compiler
- **THEN** the previous source and manifest/lockfile format still work.
