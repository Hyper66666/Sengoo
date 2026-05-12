## ADDED Requirements

### Requirement: Incremental compilation SHALL classify edits by semantic impact
The compiler SHALL classify edits as `noop`, `impl_only`, or `interface_change` to scope recomputation.

#### Scenario: No-op edit avoids unnecessary recompilation
- **WHEN** a source edit is classified as `noop`
- **THEN** dependent modules are not invalidated for recompilation

#### Scenario: Interface change invalidates dependents
- **WHEN** a source edit is classified as `interface_change`
- **THEN** dependent modules are invalidated according to dependency graph rules

### Requirement: Module fingerprints SHALL drive precise invalidation
The build system SHALL use module fingerprints to invalidate only affected compilation artifacts.

#### Scenario: Fingerprint match reuses prior artifact
- **WHEN** a module fingerprint is unchanged between builds
- **THEN** prior compilation artifacts for that module are reused
