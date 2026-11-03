## ADDED Requirements

### Requirement: The toolchain SHALL distribute packages and prove external consumption

The toolchain SHALL provide the following reproducible package distribution and independent consumption evidence.

#### Scenario: Stage deliverables are reviewed

- **WHEN** this stage is considered for completion
- **THEN** the following implementation and evidence obligations are satisfied:

- Build deterministic official package archives, checksums and a portable local registry bundle.
- Publish a curated package catalog with versions, prerequisites, supported hosts and examples.
- Run two consumer projects outside the checkout including tool-free wheel installation.
- Integrate required new gates into the existing same-revision four-host release evidence.
- Prepare a separate human pilot protocol and record actual user observations without fabricating results.

#### Scenario: A bundle consumer moves the project outside the compiler checkout

- **WHEN** a bundle consumer moves the project outside the compiler checkout
- **THEN** locked package commands and final installed artifacts do not depend on source checkout paths.
- **AND** existing compatible v0.2 projects SHALL retain their documented behavior.
