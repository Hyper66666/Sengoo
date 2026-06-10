# sgpm-package-graph Specification

## Purpose
TBD - created by archiving change ecosystem-toolchain-maturity. Update Purpose after archive.
## Requirements
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

### Requirement: Registry metadata exposes yanked versions and feature flags

`sgpm metadata --format json` SHALL expose `yanked` boolean and `features` string
lists per resolved package version without conflating them with package identity.

#### Scenario: Metadata lists yanked status separately from version identity

- **WHEN** a registry index marks `foo 1.2.3` as yanked
- **THEN** metadata JSON includes `"yanked": true` on that version entry
- **AND** package `id` fields remain `(name, version, source)` from lockfile v2 rules

#### Scenario: Fresh resolve rejects newly selected yanked versions

- **WHEN** `sgpm update` would newly select a yanked registry version
- **THEN** resolution fails with an actionable diagnostic naming the package, version,
  and yank reason when available
- **AND** locked graphs that already pin a yanked version continue to build with a
  warning until explicitly updated

### Requirement: Publish workflow documents public registry readiness

Sengoo SHALL document a publish checklist for default registry uploads covering
auth, feature manifest validation, and post-publish `sgpm metadata` verification.

#### Scenario: Quickstart includes publish checklist

- **WHEN** a maintainer opens `docs/sgpm-quickstart.md`
- **THEN** it includes a publish checklist section with required commands and
  verification steps
- **AND** the checklist references `sgpm metadata --format json` fields introduced
  by this change

### Requirement: Package publish artifacts SHALL be deterministic and inspectable

`sgpm publish --dry-run --locked` SHALL create deterministic package artifacts
with checksum sidecars and optional machine-readable metadata suitable for CI.

#### Scenario: Dry-run publish creates a deterministic artifact

- **WHEN** a maintainer runs
  `sgpm publish --dry-run --locked --output target/package`
- **THEN** sgpm validates the selected manifest and current lockfile before
  packaging
- **AND** it writes `<name>-<version>.tar.gz` and
  `<name>-<version>.tar.gz.sha256`
- **AND** the checksum is the sha256 of the archive bytes
- **AND** repeated dry-runs over unchanged package content produce the same
  checksum on supported hosts

#### Scenario: Package archive contents are bounded and safe

- **WHEN** sgpm builds the package archive
- **THEN** it includes package source, `Sengoo.toml`, selected `[bin]`/`[lib]`
  entry files, and package docs such as `README*` or `LICENSE*` when present
- **AND** it excludes build output, VCS metadata, package output directories,
  registry cache/staging directories, and host temp files
- **AND** tar paths are normalized relative paths that cannot escape the package
  root

#### Scenario: Publish metadata is machine-readable

- **WHEN** a user requests machine-readable publish output
- **THEN** output includes schema version, package name/version, selected
  manifest path, archive path, checksum path, sha256, included/excluded file
  counts, lockfile path/status, selected workspace package when any, and
  registry name when publishing to a registry
- **AND** unknown future fields are additive and do not change the meaning of
  schema version `1`

### Requirement: Registry publish SHALL be atomic and diagnosable

`sgpm publish --registry <name> --locked` SHALL publish to configured local or
remote registries without corrupting resolvable package state and without
leaking credentials.

#### Scenario: Local registry publish is atomic

- **WHEN** a package is published to a configured local file registry
- **THEN** sgpm stages files outside the final version directory and atomically
  finalizes the version directory
- **AND** a failed publish cleans staging files when possible
- **AND** `sgpm update` and locked commands can resolve the newly published
  version only after finalization

#### Scenario: Duplicate package version is rejected

- **WHEN** the target registry already contains the same package name and version
- **THEN** publish fails before overwriting existing content
- **AND** the diagnostic names the package, version, registry, and target path or
  endpoint

#### Scenario: Remote registry publish protects credentials

- **WHEN** a registry uses `[registries.<name>].url` and optional `token_env`
- **THEN** sgpm sends the token only as the documented authorization mechanism
- **AND** errors, logs, JSON output, and diagnostics do not print the token value
- **AND** HTTP status, checksum mismatch, unavailable registry, and malformed
  response failures produce stable diagnostics

#### Scenario: Published packages remain visible to metadata and cache repair

- **WHEN** a package is published and later consumed through `sgpm update`
- **THEN** `sgpm metadata --format json --locked` reports package identity,
  source id, alias edges, yanked status, and feature lists consistently
- **AND** corrupt or incomplete cached packages are detected and repairable
  through the documented refresh path

### Requirement: Release fixture SHALL prove package graph defaults

Sengoo SHALL include a package-shaped release fixture that proves aliases,
multiple package versions, registry resolution, metadata, publish dry-run, local
registry publish, and locked commands through public `sgpm` commands.

#### Scenario: Realworld package-release loop passes

- **WHEN** the realworld release fixture is run with real toolchain binaries
- **THEN** `sgpm update`, `sgpm metadata --format json --locked`,
  `sgpm publish --dry-run --locked`, `sgpm publish --registry local --locked`,
  `sgpm check --locked`, `sgpm test --locked`, `sgpm fmt --check --locked`,
  `sgpm doc --locked`, and `sgpm build --locked` pass
- **AND** the fixture includes at least one dependency alias, two resolved
  versions of one package name, and one local registry dependency
- **AND** locked commands do not rewrite `Sengoo.lock`

