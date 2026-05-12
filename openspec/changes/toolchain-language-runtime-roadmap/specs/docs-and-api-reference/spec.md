## ADDED Requirements

### Requirement: Sengoo SHALL provide maintained end-user tutorial documentation
Project documentation SHALL include an up-to-date tutorial path that covers language basics and core workflows.

#### Scenario: Tutorial covers getting-started workflow
- **WHEN** a new user follows the official tutorial
- **THEN** they can install tools, run a sample project, and understand core syntax paths

### Requirement: Toolchain SHALL provide API documentation generation
The project SHALL provide an API documentation generation workflow analogous to rustdoc-style output.

#### Scenario: API docs command generates browsable output
- **WHEN** the API documentation generation command is executed
- **THEN** a browsable API reference artifact is produced for project modules

### Requirement: Documentation SHALL include runnable example coverage
Documentation and examples SHALL include representative runnable samples for core language and tooling features.

#### Scenario: Example set validates in CI
- **WHEN** documentation examples are validated in automation
- **THEN** failing or outdated examples are reported as CI failures
