## MODIFIED Requirements

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
