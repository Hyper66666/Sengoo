## ADDED Requirements

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

