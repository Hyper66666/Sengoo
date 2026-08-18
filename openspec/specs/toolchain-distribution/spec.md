# toolchain-distribution Specification

## Purpose
Define the supported Sengoo toolchain distribution channel: versioned
Windows/Linux archives, checksum-verified installation, release smoke
gating, and coherent tool version reporting.
## Requirements
### Requirement: Toolchain releases SHALL ship versioned, checksummed archives

Tagged releases SHALL publish native toolchain archives for Windows x64,
Linux x64, macOS x64, and macOS arm64 containing `sgc`, `sgpm`, `sgfmt`,
`sglsp`, the bundled standard library/runtime bridge files needed by installed
tools, a manifest, license, and distribution README. Each archive SHALL have a
SHA-256 checksum and independently verifiable provenance, and the tag SHALL
match the coherent workspace/tool/runtime version.

#### Scenario: A tag produces the complete target set

- **WHEN** a release tag matching the workspace version is pushed
- **THEN** the packaging workflow builds release binaries on native runners for
  all four pinned targets
- **AND** publishes one archive plus one `.sha256` per target containing all
  four tools, bundled stdlib/runtime bridge files, `manifest.json`, license, and
  distribution README
- **AND** publishes provenance that identifies the source commit and workflow
- **AND** the distribution README states pinned host toolchain requirements

#### Scenario: Installed tools do not require a source checkout

- **WHEN** a developer installs any supported archive outside the repository
  checkout
- **THEN** `sgc` resolves bundled `std::*` modules and runtime bridge sources by
  default
- **AND** repository paths and `SENGOO_ROOT`-style variables are optional
  overrides rather than hidden requirements

#### Scenario: A mismatched tag or incomplete target set fails fast

- **WHEN** the tag differs from the coherent workspace version or any required
  target fails packaging, checksum, provenance, or installed smoke
- **THEN** the workflow fails publication for the release
- **AND** it does not present a partial target set as a successful release

#### Scenario: A tag produces both target archives

- **WHEN** a release tag matching the workspace version is pushed
- **THEN** the packaging workflow builds release binaries on native runners
  for both pinned targets
- **AND** publishes one archive plus one `.sha256` per target containing
  all four tools, the bundled stdlib/runtime bridge files, `manifest.json`,
  the license, and the distribution README
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
- **AND** the dry-run writes release-shaped artifacts under `target/dist/`
  so the same install-script and transcript checks can consume them

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
- **AND** a stdlib-import smoke succeeds without pointing the script or tool
  at the source checkout

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
