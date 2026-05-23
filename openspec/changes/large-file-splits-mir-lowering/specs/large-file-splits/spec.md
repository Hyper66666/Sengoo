## ADDED Requirements

### Requirement: Existing child directory root splitting

Large-file split changes MUST preserve existing child helper modules when converting a large module root file into `mod.rs`; the split MUST keep the same logical module path for the parent and all existing children, and MUST avoid leaking parent-private implementation details outside the parent module boundary.

#### Scenario: Existing child helper paths remain stable

- **GIVEN** a module root file `foo.rs` declares child modules that already resolve to files under `foo/*.rs`
- **WHEN** the root file is converted to `foo/mod.rs`
- **THEN** each existing child module MUST continue resolving under the same logical parent module path
- **AND** callers outside the parent module MUST NOT need to update imports because of the physical file move.

#### Scenario: Parent-private context remains contained

- **GIVEN** the original root file owns a private context type whose fields and helper methods are used by child helper modules
- **WHEN** root methods are split into sibling files under the same directory module
- **THEN** private fields and helper methods MUST remain no more visible than required by Rust privacy
- **AND** any necessary helper method promotion MUST stop at `pub(super)` unless the item was already part of the public API.

#### Scenario: Existing helper tests keep compiling

- **GIVEN** existing child helper modules contain unit tests that instantiate root-owned helper types or structs
- **WHEN** the root file is split into `mod.rs` plus sibling helper files
- **THEN** those tests MUST keep compiling without changing their asserted behavior
- **AND** test-only imports MAY be adjusted only to preserve access to the same logical items.
