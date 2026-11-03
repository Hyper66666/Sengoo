## ADDED Requirements

### Requirement: The toolchain SHALL verify ownership composition and native artifact builds

The toolchain SHALL meet the following ownership and native artifact correctness requirements.

#### Scenario: Stage deliverables are reviewed

- **WHEN** this stage is considered for completion
- **THEN** the following implementation and evidence obligations are satisfied:

- Run owning aggregate/generic/borrow/control-flow/async cleanup composition tests and correct discovered defects.
- Add explicit shared-library builds whose selected exports are reachability roots.
- Link runtime and package sources with correct PIC and explicit export boundaries.
- Invalidate native object caches on source/transitive header/compiler/target/options/output-mode changes.
- Keep existing source/ABI/diagnostic compatibility and document rejected boundaries.

#### Scenario: A header or target or build mode changes

- **WHEN** a header or target or build mode changes
- **THEN** a dependent native object is rebuilt before it can be linked.
- **AND** existing compatible v0.2 projects SHALL retain their documented behavior.
