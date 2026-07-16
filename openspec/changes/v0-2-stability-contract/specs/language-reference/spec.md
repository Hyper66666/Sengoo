## ADDED Requirements

### Requirement: The language reference SHALL classify stability per public surface

Every documented language, stdlib, tool, and backend surface SHALL be marked
Stable, Supported subset, Experimental, or Deprecated for the referenced
toolchain version, with proof and compatibility meaning.

#### Scenario: A construct is marked Stable

- **WHEN** the reference marks a construct Stable for v0.2
- **THEN** executable proof covers its positive, negative, and lifecycle behavior
- **AND** later v0.2.x releases preserve that contract

#### Scenario: A construct is Experimental

- **WHEN** the reference marks a construct Experimental
- **THEN** it states the unsupported/default-path boundary
- **AND** no release or support-matrix summary presents it as mainstream Stable

### Requirement: Edition and migration behavior SHALL be explicit

The reference SHALL state that v0.2 uses edition 2026, identify any v0.1 -> v0.2
source changes, and link each breaking or deprecated behavior to migration
guidance and tests.

#### Scenario: User upgrades a v0.1 package

- **WHEN** a documented behavior changed for v0.2
- **THEN** the migration guide shows old/new code and the diagnostic or edition
  behavior encountered
- **AND** a retained fixture prevents the guidance from drifting
