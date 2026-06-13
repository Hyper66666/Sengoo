## ADDED Requirements

### Requirement: A package registry protocol and reference server SHALL exist

The toolchain SHALL define an HTTP registry protocol (publish, yank, version
listing, metadata, download) with checksum verification and name reservation,
and SHALL provide a reference server implementing it.

#### Scenario: Publish and download through the registry

- **WHEN** a package is published to the reference registry and another project
  downloads it
- **THEN** the upload succeeds with a recorded checksum
- **AND** the download is checksum-verified before use
- **AND** publishing a name already reserved by another owner is rejected

### Requirement: Dependencies SHALL resolve from a registry with hash-locked entries

`sgpm` SHALL resolve semver version requirements against a registry and SHALL
record content-hashed lockfile entries deterministically.

#### Scenario: Registry dependency build is reproducible

- **WHEN** a project depends on a registry package by semver and is built twice
- **THEN** the resolver selects the same version both times
- **AND** the lockfile records a content hash that is verified on the second build
- **AND** path and git dependencies continue to resolve as before

### Requirement: The toolchain SHALL be distributed as versioned binaries

The project SHALL publish versioned, checksummed toolchain artifacts for Linux
x86_64, Windows x64, and macOS, installable via one command.

#### Scenario: Install and run on each platform

- **WHEN** a user installs a released toolchain on Linux, Windows, or macOS via
  the documented one-command installer
- **THEN** the install is checksum-verified
- **AND** `sgc run` on a hello-world program succeeds on that platform per the
  release smoke matrix
