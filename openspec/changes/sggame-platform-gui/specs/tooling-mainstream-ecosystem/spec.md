## ADDED Requirements

### Requirement: Graphics ecosystem packages SHALL publish through sgpm as third-party-style packages

The repository SHALL treat `sgplatform`, `sggame`, and `sggui` as sgpm packages
under `packages/` with manifests, lockfiles, tests, and documentation rather
than as ad hoc examples or premature `tools/stdlib/` modules.

Graphics package builds SHALL use the existing package manifest schema in this
change. Native SDL2 libraries SHALL be carried by source-level FFI link metadata
and documented environment/toolchain setup rather than a new manifest section.

#### Scenario: Each graphics package is sgpm-shaped

- **WHEN** a user inspects `packages/sgplatform`, `packages/sggame`, and
  `packages/sggui`
- **THEN** each directory contains `Sengoo.toml`, source entry files, tests,
  and README instructions
- **AND** dependency edges declare `sggame -> sgplatform` and `sggui ->
  sgplatform`

#### Scenario: Locked package loop applies to graphics packages

- **WHEN** CI or a user runs `sgpm update` followed by `sgpm test --locked` and
  `sgpm build --locked` inside a graphics package on a supported host
- **THEN** the commands succeed or record an accepted platform skip documented
  in the graphics support matrix
- **AND** stale lockfiles are rejected before invoking `sgc` where locked mode
  is used

#### Scenario: Native link schema is not invented by graphics packages

- **WHEN** a reviewer inspects `packages/*/Sengoo.toml` for the graphics packages
- **THEN** the manifests use existing package, dependency, target, and test
  fields only
- **AND** any need for manifest-level native libraries is documented as a
  follow-up OpenSpec rather than implemented ad hoc

### Requirement: Graphics packages SHALL be discoverable from repository docs

The examples or packages documentation SHALL link to `sgplatform`, `sggame`, and
`sggui` quickstarts and to the graphics support matrix.

#### Scenario: User discovers graphics packages from the repo root docs

- **WHEN** a user reads the examples or packages index linked from README
- **THEN** they can find entry points for blank window, snake, and counter demos
- **AND** they can find native dependency installation instructions
