## ADDED Requirements

### Requirement: sgpm SHALL be the canonical Sengoo package manager interface
The package manager CLI and documentation SHALL use the name `sgpm` as the primary interface.

#### Scenario: Package command references sgpm
- **WHEN** package operations are invoked from official Sengoo docs or toolchain help
- **THEN** command examples and references use `sgpm` naming

### Requirement: sgpm SHALL use Sengoo.toml for package metadata and dependencies
`sgpm` SHALL read project package metadata and dependency declarations from `Sengoo.toml`.

#### Scenario: Dependency metadata is loaded from manifest
- **WHEN** a project contains `Sengoo.toml` with dependency entries
- **THEN** `sgpm` resolves dependencies based on that manifest

### Requirement: sgpm SHALL support semantic-version dependency constraints and conflict reporting
Dependency resolution SHALL implement semantic-version matching and SHALL fail with actionable diagnostics on unsatisfiable constraints.

#### Scenario: Version conflict is reported clearly
- **WHEN** two dependencies require incompatible versions of the same package
- **THEN** `sgpm` fails resolution and reports the conflicting constraints

### Requirement: sgpm SHALL support private registries
`sgpm` SHALL support package fetch and publish workflows against configured private registries.

#### Scenario: Private registry dependency is resolved
- **WHEN** project dependency source points to an authenticated private registry
- **THEN** `sgpm` retrieves package metadata and artifacts from that registry
