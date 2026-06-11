## Context

Release engineering assets today: CI build/test workflows, perf and
realworld-e2e jobs, and `docs/internal-release.md` defining a smoke matrix
(versioned binaries, smoke commands, rollback) without automation. The
umbrella froze decision D5: two archives (win-x64 zip, linux-x64 tar.gz),
checksums, smoke-gated tag-triggered publication, non-auto-updating install
scripts, and workspace-sourced version coherence.

## Decisions

### D-D1 Artifact layout

```text
sengoo-<version>-windows-x64.zip
  bin/sgc.exe  bin/sgpm.exe  bin/sgfmt.exe  bin/sglsp.exe
  share/sengoo/stdlib/**  share/sengoo/runtime/**
  LICENSE  README-dist.md  manifest.json
sengoo-<version>-linux-x64.tar.gz
  bin/sgc  bin/sgpm  bin/sgfmt  bin/sglsp
  share/sengoo/stdlib/**  share/sengoo/runtime/**
  LICENSE  README-dist.md  manifest.json
sengoo-<version>-<target>.{zip,tar.gz}.sha256
```

- `<version>` is `workspace.package.version`; the tag must equal
  `v<version>` or the workflow fails fast.
- `README-dist.md` states the pinned `clang`/LLVM requirement from the
  toolchain contract (`codegen-ir-correctness-and-gate`) because `sgc`
  still drives the host `clang` for native builds.
- The archive is self-contained for standard-library and runtime bridge
  lookup. Installed tools must not require a source checkout or a manually
  set `SENGOO_ROOT` to import `std::*` modules or link the bundled runtime C
  bridge sources. If an environment override remains supported, it is an
  override, not the default lookup path.
- `manifest.json` records target, version, git hash, build time, runner OS
  image, tool binary names, bundled stdlib/runtime bundle hash, archive
  filename, and SHA-256 checksum.

### D-D2 Workflow gating

- Trigger: pushing tag `v*`. Steps per target runner: checkout → pinned
  Rust toolchain (`rust-toolchain.toml`) → `cargo build --release` →
  run the `docs/internal-release.md` smoke matrix against the built
  binaries (`sgc run examples/01_hello.sg`, one stdlib import smoke,
  `sgpm new/check/build` loop, `sgfmt --check`, `sglsp` handshake smoke)
  → package → checksum → upload to the GitHub release for the tag.
- Any smoke failure on any target blocks publication of all artifacts for
  that tag (no partial releases).
- A `workflow_dispatch` dry-run mode builds and smokes without publishing
  (used by verification task 6.10 of the umbrella). Dry-run artifacts are
  written under `target/dist/<version>/` with the same archive names,
  `manifest.json`, and `.sha256` files as a real release.

### D-D3 Install scripts

- `scripts/install.ps1` and `scripts/install.sh`: parameters are version
  (required, no "latest" auto-resolution in v1), install dir (default
  `~/.sengoo` or `%USERPROFILE%\.sengoo`), and optional repository/source
  URL for internal mirrors.
- Behavior: download archive + `.sha256` → verify checksum (hard fail on
  mismatch) → extract to versioned dir → place/refresh PATH shims from
  `<install-dir>/bin` or print the exact PATH instruction → run
  `sgc --version` as the success check.
- Never auto-update, never elevate, never modify shell profiles without an
  explicit `-AddToPath`/`--add-to-path` flag.

### D-D4 Version coherence

- Single source: `workspace.package.version` + `SENGOO_BUILD_HASH` (git
  short hash) injected at build time (build script env, with `unknown`
  fallback for non-git builds).
- All four tools expose `--version` printing
  `<tool> <version> (<short-hash>)`; a workspace test asserts the four
  strings share version and hash.

### D-D5 Claim discipline

- The SUPPORT_MATRIX distribution row claims exactly: versioned win-x64 and
  linux-x64 archives with checksums, smoke-gated publication, script
  install, and version coherence — citing the workflow file, a release tag
  dry-run log, and the fresh-host transcript.
- macOS stays out of the claim; the row records it as deferred.
- The fresh-host transcript must prove both a trivial program and a stdlib
  import from the installed distribution, so a release cannot accidentally
  depend on the repository checkout's `tools/stdlib`.

## Risks / Trade-offs

- Distribution makes CLI breakage user-visible; the smoke gate plus tag ==
  version check are mandatory blockers, not advisories.
- GitHub runner images drift; pinning the Rust toolchain file and recording
  runner image versions in the release log keeps builds attributable.
- `sgc` still requires a host `clang`: the dist README must say so
  prominently, or installs "succeed" but native builds fail confusingly —
  the fresh-host transcript covers this path explicitly.

## Migration Plan

Additive. Source builds keep working; docs gain the install path as the
recommended route.

## Open Questions

- Hosting of archives beyond GitHub Releases (internal mirror) is deferred
  to operations; the spec only requires the documented channel to work.
