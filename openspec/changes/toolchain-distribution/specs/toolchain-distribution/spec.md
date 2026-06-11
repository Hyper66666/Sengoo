## ADDED Requirements

### Requirement: Toolchain releases SHALL ship versioned, checksummed archives

Tagged releases SHALL publish toolchain archives for Windows x64 and Linux
x64 containing `sgc`, `sgpm`, `sgfmt`, and `sglsp` plus license and
distribution README, each with a SHA-256 checksum file, where the tag
matches the workspace version.

#### Scenario: A tag produces both target archives

- **WHEN** a release tag matching the workspace version is pushed
- **THEN** the packaging workflow builds release binaries on native runners
  for both pinned targets
- **AND** publishes one archive plus one `.sha256` per target containing
  all four tools, the license, and the distribution README
- **AND** the distribution README states the pinned host `clang`/LLVM
  requirement

#### Scenario: A mismatched tag fails fast

- **WHEN** the tag does not equal the workspace version
- **THEN** the workflow fails before building or publishing anything

### Requirement: Publication SHALL be gated on the documented smoke matrix

The packaging workflow SHALL run the documented internal-release smoke
matrix against the built binaries on every target and SHALL block all
publication for the tag when any smoke fails.

#### Scenario: A smoke failure blocks the release

- **WHEN** any smoke command fails on any target during the release run
- **THEN** no artifact for that tag is published
- **AND** the failure names the target and command

#### Scenario: A dry-run validates without publishing

- **WHEN** the workflow runs in its dispatch dry-run mode
- **THEN** it builds, smokes, and packages both targets without publishing
- **AND** the produced archives pass checksum verification

### Requirement: Install scripts SHALL verify and install a pinned version

Documented install scripts for Windows and POSIX SHALL download a pinned
version, verify the archive checksum before installing, place the tools on
PATH per their documented mode, and never auto-update or modify shell
profiles without an explicit opt-in flag.

#### Scenario: A fresh host installs and runs

- **WHEN** a developer runs the install script with a pinned version on a
  fresh supported host
- **THEN** the script verifies the checksum, installs all four tools, and
  confirms success via the tool version output
- **AND** with a host `clang` present, `sgc run examples/01_hello.sg`
  succeeds with the documented result

#### Scenario: A checksum mismatch aborts the install

- **WHEN** the downloaded archive does not match its published checksum
- **THEN** the script aborts without installing anything and reports the
  mismatch

### Requirement: Released tools SHALL report one coherent version

All four released tools SHALL report the same version string sourced from
the workspace version plus the built revision hash.

#### Scenario: Versions agree across tools

- **WHEN** a developer queries `--version` on `sgc`, `sgpm`, `sgfmt`, and
  `sglsp` from one release
- **THEN** all four report the same workspace version and revision hash
- **AND** an automated test asserts this coherence
