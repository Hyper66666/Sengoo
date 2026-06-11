## Why

The only way to obtain the Sengoo toolchain is to clone the repository and
run `cargo build --release`. `docs/internal-release.md` (Pillar 6 of
`six-pillar-gap-closure`) defines a versioned-binary smoke matrix and
rollback procedure, but it is documentation only: no packaging workflow, no
published archives, no checksums, no install scripts, and unverified
`--version` coherence across `sgc`, `sgpm`, `sgfmt`, and `sglsp`. A
language nobody can install without building its compiler from source
cannot be adopted. This is Pillar D of the
`mainstream-adoption-gap-closure` umbrella.

## What Changes

- Add a tag-triggered packaging workflow that builds, smoke-tests, and
  publishes versioned toolchain archives for Windows x64
  (`sengoo-<version>-windows-x64.zip`) and Linux x64
  (`sengoo-<version>-linux-x64.tar.gz`), each containing `sgc`, `sgpm`,
  `sgfmt`, `sglsp`, the license, and a pinned-toolchain README, plus a
  `.sha256` per archive.
- Gate publication on the `docs/internal-release.md` smoke matrix: a failed
  smoke blocks the release.
- Add install scripts (`install.ps1`, `install.sh`) that download a pinned
  version, verify the checksum, install binaries onto PATH, and never
  auto-update.
- Make all four tools report one coherent version string sourced from
  `workspace.package.version` plus the built git short hash, with tests.
- Document the install path in the READMEs and add a distribution row to
  `examples/realworld/SUPPORT_MATRIX.md`.

## Capabilities

### New Capabilities

- `toolchain-distribution`: the versioned-artifact layout, checksum and
  smoke gating, install-script contract, and cross-tool version coherence.

### Modified Capabilities

- None. `tooling-mainstream-ecosystem` (internal release docs) is consumed
  as the gate definition, not re-specified.

## Impact

- `.github/workflows/` (new packaging/release workflow), `scripts/`
  (install scripts, packaging helpers), `tools/sgc`, `tools/sgpm`,
  `tools/sgfmt`, `tools/sglsp` (version plumbing), `README.md`,
  `README.zh-CN.md`, `docs/internal-release.md` (channel pointers),
  `examples/realworld/SUPPORT_MATRIX.md`.
- Parent umbrella: `mainstream-adoption-gap-closure` (Pillar D); decision
  D5 is frozen there.
- Runs in parallel with Pillar A (disjoint surfaces); no dependency on
  Pillars B/C.

## Non-Goals

- No macOS target in this wave; the workflow layout must not preclude
  adding one later.
- No auto-updater, package-manager listings (winget/apt/brew), or public
  registry hosting.
- No nightly channel; tagged versions only.
- No cross-compilation guarantees beyond the two pinned host targets built
  on their native runners.
