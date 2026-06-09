## ADDED Requirements

### Requirement: Internal toolchain releases SHALL have auditable smoke and rollback

Sengoo SHALL make internal toolchain releases auditable by building the full
tool set, recording archive metadata/checksums, running realworld smoke, and
documenting rollback.

#### Scenario: Release smoke builds the full tool set

- **WHEN** a release candidate is prepared on a supported host
- **THEN** the release smoke builds `sgc`, `sgpm`, `sgfmt`, and `sglsp` in the
  selected release profile
- **AND** the smoke runs realworld locked package loops with real binaries
- **AND** the smoke includes `sglsp` realworld diagnostics or documents an
  evidenced host/tooling skip

#### Scenario: Release archive has a manifest and checksums

- **WHEN** the release archive is assembled
- **THEN** its manifest records tool versions, git SHA, host triple, bundled
  stdlib/runtime contents, archive filename, and sha256 checksums
- **AND** `docs/internal-release.md` documents how maintainers verify the archive
  before tagging

#### Scenario: Rollback verifies package compatibility

- **WHEN** a maintainer rolls back to a previous toolchain archive
- **THEN** the documented rollback runs `sgpm update --check` and the locked
  package loop before declaring the rollback healthy
- **AND** any lockfile incompatibility produces an actionable diagnostic rather
  than silently rewriting lockfiles

#### Scenario: Quickstart documents release package workflow

- **WHEN** a maintainer opens `docs/sgpm-quickstart.md`
- **THEN** it shows deterministic publish dry-run, local registry publish,
  remote registry credential guidance, and
  `sgpm metadata --format json --locked` verification
- **AND** examples avoid leaking registry tokens in commands or expected output

