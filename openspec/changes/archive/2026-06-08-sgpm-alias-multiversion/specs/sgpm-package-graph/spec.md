## ADDED Requirements

### Requirement: Superseded package-graph delta SHALL have one canonical owner

The superseded package graph requirements SHALL be historical implementation evidence after
`ecosystem-toolchain-maturity` copies them forward.

#### Scenario: Reviewer follows the canonical package-graph owner

- **WHEN** a reviewer needs the active package graph requirements
- **THEN** they use
  `openspec/changes/ecosystem-toolchain-maturity/specs/sgpm-package-graph/spec.md`
- **AND** this change is not used as the active canonical owner

### Requirement: Dependency aliases resolve through an explicit package field

`sgpm` SHALL resolve dependency keys that differ from `[package].name` only when
the dependency table includes `package = "actual_name"`.

#### Scenario: Renamed dependency keys resolve to the correct package

- **WHEN** `Sengoo.toml` contains:

```toml
[dependencies.my_alias]
package = "actual_name"
path = "../actual_name"
```

- **THEN** `sgpm update` resolves `my_alias` to package `actual_name`
- **AND** `sgpm check` and `sgpm build` compile against the correct sources
- **AND** import/module-map diagnostics use the alias key without requiring it to
  equal `[package].name`

### Requirement: Multiple versions of one package name coexist in one graph

`sgpm` SHALL allow a workspace graph to contain distinct resolved versions of the
same package name when manifest and source constraints permit it.

#### Scenario: Two versions of one package coexist in one graph

- **WHEN** a workspace requires `foo 1.0.0` and `foo 2.0.0` through distinct
  allowed dependency paths
- **THEN** `sgpm update` writes a lockfile that contains distinct package nodes for
  both versions
- **AND** conflicting source requirements for the same `(name, version, source)`
  tuple still fail with an actionable diagnostic

### Requirement: Lockfile version 2 SHALL freeze package identity and edge aliases

`Sengoo.lock` version 2 SHALL use this shape:

```toml
version = 2

[[package]]
id = "foo@1.0.0+path:../foo"
name = "foo"
version = "1.0.0"
source.kind = "path"
source.path = "../foo"

[[dependency]]
from = "app@0.1.0+path:."
alias = "my_alias"
to = "foo@1.0.0+path:../foo"
```

Rules:

- Package node identity is `(name, version, source)` and is encoded in `id` using
  canonical slash-normalized source strings.
- `alias` is edge metadata only and MUST NOT appear in package `id`.
- Source canonicalization: paths are repo-relative with `/` separators; git sources
  use `git+<url>#<rev>`; registry sources use `registry+<name>@<version>`.

#### Scenario: Version 1 lockfiles remain readable for compatible graphs

- **WHEN** a version 1 lockfile represents a graph without dependency aliases or
  multiple versions of the same package name
- **THEN** locked commands may read it without rewriting it

#### Scenario: sgpm update migrates compatible graphs to version 2

- **WHEN** `sgpm update` runs against a compatible version 1 lockfile
- **THEN** it rewrites the lockfile deterministically to version 2
- **AND** locked/check/build/test commands never rewrite lockfiles

#### Scenario: Incompatible graphs fail before locked commands run

- **WHEN** the selected manifest and resolver graph require dependency aliases or
  multiple versions of the same package name but the on-disk lockfile is version 1
- **THEN** locked commands fail with an actionable diagnostic that names the
  manifest and instructs `sgpm update`
- **AND** the failure is based on graph expressibility, not on incidental formatting
  of the version 1 file alone

### Requirement: Metadata JSON exposes alias resolution

`sgpm metadata --format json` SHALL list canonical package identity separately from
dependency-edge `alias`, `from`, and `to` fields.

#### Scenario: Metadata lists aliases and package identities separately

- **WHEN** a user runs `sgpm metadata --format json` for a graph with aliased
  dependencies
- **THEN** each package entry lists canonical `name`, `version`, and `source`
- **AND** dependency edges expose `alias`, `from`, and `to` fields using the
  lockfile v2 identity format
