## 1. Inventory

- [x] 1.1 Run `openspec validate package-release-defaults --strict`.
- [x] 1.2 Reconfirm archived package graph ownership:
  `sgpm-alias-multiversion`, `ecosystem-toolchain-maturity`, and
  `toolchain-internal-ux` remain the baseline owners for resolver, metadata, and
  first realworld loop behavior.
- [x] 1.3 Update `mainstream-default-readiness` / `mainstream-production-readiness`
  inventories to point P2/ecosystem release work at this child change.

## 2. Deterministic Package Artifacts

- [x] 2.1 Ensure `sgpm publish --dry-run --locked --output <dir>` validates the
  selected manifest and lockfile before packaging.
- [x] 2.2 Make package archives deterministic: stable file ordering, normalized
  tar paths, normalized permissions, stable gzip metadata, and excluded publish
  output/cache/VCS/build directories.
- [x] 2.3 Ensure dry-run creates `<name>-<version>.tar.gz` and
  `<name>-<version>.tar.gz.sha256` with sha256 of the archive bytes.
- [x] 2.4 Add `--format json` or equivalent schema-versioned machine output for
  dry-run/publish metadata, including package identity, archive path, checksum,
  file counts, lockfile status, registry name, and workspace package when
  selected.
- [x] 2.5 Add tests that two dry-runs over unchanged content produce identical
  checksums, and that a content change changes the checksum.

## 3. Registry Publish And Cache Evidence

- [x] 3.1 Add local registry publish tests for success, duplicate version
  rejection, staging cleanup after failure, and subsequent locked resolution.
- [x] 3.2 Add remote registry publish tests with a local test server covering
  auth token use, token redaction in diagnostics/logs, HTTP failures, checksum
  mismatch, and index/download compatibility with `sgpm update`.
- [x] 3.3 Add cache refresh/repair tests proving published remote packages can be
  consumed, detected as corrupt/incomplete, and repaired without stale metadata.
- [x] 3.4 Ensure yanked/features metadata remains visible through
  `sgpm metadata --format json --locked` after publish/resolve.

## 4. Realworld Release Fixture

- [x] 4.1 Add `examples/realworld/package-release-loop` or an equivalent fixture
  that uses dependency alias, two resolved versions of one package name, and a
  local registry dependency.
- [x] 4.2 Run the fixture through `sgpm update`, `metadata --format json --locked`,
  `publish --dry-run --locked`, `publish --registry local --locked`,
  `check/test/fmt/doc/build --locked`.
- [x] 4.3 Update `examples/realworld/SUPPORT_MATRIX.md` with package-release
  proof paths and no stale package/release Deferred row.

## 5. Toolchain Release Channel

- [x] 5.1 Update `.github/workflows/realworld-e2e.yml` or release docs so release
  smoke builds `sgc`, `sgpm`, `sgfmt`, and `sglsp`.
- [x] 5.2 Add `sglsp` realworld diagnostic/stdlib/package fixture smoke to the
  release path.
- [x] 5.3 Update `docs/internal-release.md` with archive manifest fields,
  checksum verification, bundled stdlib/runtime contents, installation path,
  rollback steps, and required smoke commands.
- [x] 5.4 Update `docs/sgpm-quickstart.md` with deterministic dry-run, local
  registry publish, remote registry credential guidance, and metadata
  verification commands.

## 6. Verification

- [x] 6.1 `cargo test -p sgpm publish`
- [x] 6.2 `cargo test -p sgpm metadata`
- [x] 6.3 `cargo test -p sgpm realworld -- --nocapture`
- [x] 6.4 `cargo test -p sglsp realworld -- --nocapture`
- [x] 6.5 `openspec validate package-release-defaults --strict`
- [x] 6.6 `openspec validate --all --strict`

## Archive Gate

- [ ] Deterministic package archive and checksum tests pass on Windows and
  Ubuntu or record evidenced host-specific skips. Windows/local evidence passed;
  Ubuntu/reference-host evidence remains pending CI or a separate host run.
  This Windows workspace cannot execute an Ubuntu reference run locally: `wsl -l
  -v` reports no installed Linux distribution and `docker` is not available in
  PATH. `.github/workflows/realworld-e2e.yml` contains an `ubuntu-latest` matrix
  lane with dedicated `cargo test -p sgpm publish -- --nocapture` and
  `cargo test -p sgpm metadata -- --nocapture` steps, but no remote job output
  is available in this workspace to cite as passing evidence.
- [x] Local and remote publish tests cover success, duplicate/error, auth/token
  redaction, checksum, cache refresh/repair, and metadata JSON.
- [x] A realworld package-release fixture proves the default workflow through
  public `sgpm` commands.
- [x] Internal release docs and CI/release smoke include all four tools:
  `sgc`, `sgpm`, `sgfmt`, and `sglsp`.
- [x] `openspec validate package-release-defaults --strict` passes.
- [x] `openspec validate --all --strict` passes.
