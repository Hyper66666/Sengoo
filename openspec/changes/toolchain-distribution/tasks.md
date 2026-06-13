## 1. Pinning

- [x] 1.1 Run `openspec validate toolchain-distribution --strict`.
- [x] 1.2 Pin archive layout, install dir defaults, and PATH-flag names in
  `design.md` (D-D1/D-D3) before code edits.

## 2. Version coherence

- [x] 2.1 Inject `SENGOO_BUILD_HASH` at build time with `unknown` fallback;
  source version from `workspace.package.version`.
- [x] 2.2 `--version` on `sgc`, `sgpm`, `sgfmt`, `sglsp` prints
  `<tool> <version> (<short-hash>)`.
- [x] 2.3 Workspace test asserts all four tools agree on version and hash.

## 3. Packaging workflow

- [x] 3.1 Tag-triggered workflow (`v*`) with tag == version fail-fast
  check; per-target release builds on native runners with the pinned Rust
  toolchain.
- [ ] 3.2 Smoke matrix from `docs/internal-release.md` runs against built
  binaries on each target; any failure blocks all publication for the tag.
- [ ] 3.3 Package archives per D-D1 with `.sha256`; upload to the GitHub
  release; `workflow_dispatch` dry-run mode builds and smokes without
  publishing.
- [x] 3.4 Archives include bundled stdlib/runtime bridge files plus
  `manifest.json`; a dry-run extracts the archive outside the checkout and
  proves `sgc` can resolve `std::*` without `SENGOO_ROOT`.

## 4. Install scripts

- [x] 4.1 `scripts/install.sh`: download, checksum-verify (hard fail),
  extract, PATH shim or instruction, `sgc --version` success check; no
  auto-update, opt-in `--add-to-path` only.
- [x] 4.2 `scripts/install.ps1`: same contract for Windows.
- [ ] 4.3 Fresh-host (or clean-container/VM) transcript per target:
  install via script, then `sgc run examples/01_hello.sg` succeeds with a
  host `clang` present; transcript committed and linked.
- [x] 4.4 Transcript also runs one stdlib import smoke from outside the source
  checkout, proving the distribution is self-contained for stdlib/runtime
  lookup.

## 5. Docs and matrix

- [x] 5.1 `README-dist.md` in archives states the pinned `clang`/LLVM
  requirement and quickstart.
- [x] 5.2 READMEs document the install path; `docs/internal-release.md`
  points at the automated channel.
- [x] 5.3 Add the distribution row to
  `examples/realworld/SUPPORT_MATRIX.md` with workflow, dry-run log, and
  transcript links; record macOS as deferred.

## 6. Verification

- [x] 6.1 `cargo fmt --check`
- [ ] 6.2 Version coherence test green on Windows and Linux CI
- [ ] 6.3 Workflow dry-run produces installable, checksum-valid archives
  for both targets
- [x] 6.4 Dry-run archive extraction proves stdlib/runtime lookup without a
  source checkout or manual `SENGOO_ROOT`
- [ ] 6.5 Install-script transcripts committed for both targets
- [x] 6.6 `openspec validate toolchain-distribution --strict`

## Archive Gate

- [x] `openspec validate toolchain-distribution --strict` passes.
- [ ] A dry-run (or real) tagged release produced smoke-gated, checksummed
  archives for win-x64 and linux-x64.
- [ ] Script installs are proven by fresh-host transcripts ending in a
  successful `sgc run examples/01_hello.sg` plus a stdlib import smoke from
  outside the source checkout.
- [ ] All four tools report one coherent version string with tests.
- [ ] Matrix row added with proof; umbrella records Pillar D completion.
