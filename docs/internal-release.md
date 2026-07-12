# Internal toolchain release channel

Process for tagging and distributing versioned `sgc`, `sgpm`, `sgfmt`, and `sglsp` binaries to internal teams.

## Supported host policy

Internal release candidates are supported on the same hosts exercised by the
realworld workflow:

| Host | CI evidence | Notes |
| --- | --- | --- |
| Windows x64 | `windows-latest` distribution matrix | Produces `x86_64-pc-windows-msvc`. |
| Ubuntu x64 | `ubuntu-latest` distribution matrix | Produces `x86_64-unknown-linux-gnu`. |
| macOS arm64 | `macos-15` distribution matrix | Produces `aarch64-apple-darwin`. |
| macOS x64 | `macos-15-intel` distribution matrix | Produces `x86_64-apple-darwin`. |

The matrix uses native runners for every channel. Packaging receives the
explicit target label and first checks that the installer detects the same
host architecture.

## Binaries in the release set

| Tool | Crate path | Role |
| --- | --- | --- |
| `sgc` | `tools/sgc` | compile, run, test, check |
| `sgpm` | `tools/sgpm` | manifests, lockfiles, workspace commands |
| `sgfmt` | `tools/sgfmt` | formatter |
| `sglsp` | `tools/sglsp` | language server |

Build release artifacts:

```powershell
cargo build --release -p sgc -p sgpm -p sgfmt -p sglsp
.\scripts\package-toolchain.ps1 -Version 0.1.0-smoke -NoBuild
```

The packaging script writes a release-shaped archive, `manifest.json`, and a
`.sha256` sidecar under `target/dist/`. The archive includes `bin/` plus
`share/sengoo/stdlib` and `share/sengoo/runtime`, so installed `sgc` can resolve
stdlib imports without `SENGOO_ROOT` or a source checkout.

Name archives with the toolchain tag and host triple, for example
`sengoo-toolchain-2026.06.08-x86_64-pc-windows-msvc.zip`.

Each archive must include a plain-text or JSON manifest recording:

- tool versions for `sgc`, `sgpm`, `sgfmt`, and `sglsp`;
- Git SHA and release tag;
- host triple and OS image used for the build;
- archive filename and SHA-256 checksum;
- bundled `tools/stdlib/` source modules and runtime C/object files included in
  the archive;
- package smoke evidence command names and dates.

Verify the archive before tagging:

```powershell
Get-FileHash .\sengoo-toolchain-<tag>-<host>.zip -Algorithm SHA256
```

On POSIX hosts:

```bash
sha256sum sengoo-toolchain-<tag>-<host>.tar.gz
```

## Smoke tests before tagging

Run on the candidate host before tagging:

```powershell
cargo build -p sgc -p sgpm -p sgfmt -p sglsp
cargo test -p sgpm realworld -- --nocapture
cargo test -p sglsp realworld -- --nocapture
npx --yes @fission-ai/openspec validate toolchain-distribution --strict
npx --yes @fission-ai/openspec validate --all --strict
```

The `realworld-e2e` CI workflow builds real `sgc`, `sgpm`, `sgfmt`, and
`sglsp` binaries on Windows and Ubuntu, runs
`cargo test -p sgpm realworld -- --nocapture`, runs
`cargo test -p sglsp realworld -- --nocapture`, and verifies package smoke loops
for `sgplatform`, `sggame`, and `sggui` in stub graphics mode. The sgpm
realworld suite includes `examples/realworld/package-release-loop`, which covers
locked metadata, deterministic publish dry-run JSON, local registry publish,
and the locked package loop.

## Tagging

1. Ensure `npx --yes @fission-ai/openspec validate --all --strict` passes.
2. Tag the repository with the workspace version (`v<version>`). The
   `toolchain-distribution` workflow rejects tags that do not match the
   workspace version.
3. Let `.github/workflows/toolchain-distribution.yml` build, install, and smoke
   all four native archives. The release job waits for every matrix entry,
   creates GitHub build-provenance attestations, then publishes the complete
   archive/checksum set. Record the Git SHA and host triple in the internal
   change log.

## Install and explicit upgrade

The install scripts choose the current host channel, download the pinned tag,
verify its SHA-256, and replace the selected install directory:

```powershell
.\scripts\install.ps1 -Version 0.1.0 -AddToPath
```

```sh
sh scripts/install.sh --version 0.1.0 --add-to-path
```

Run the same command with a newer explicit version to upgrade. There is no
background or implicit update path. Use `-PrintTarget` or `--print-target` to
inspect auto-detection, and `-Target` or `--target` only when intentionally
selecting a different published channel.

Verify signed provenance for a downloaded release archive:

```text
gh attestation verify <archive> --repo Hyper66666/Sengoo
```

## Rollback

1. Reinstall the previous tagged archive into `PATH`.
2. Run `sgpm update --check` in affected packages to confirm lockfile compatibility.
3. Run the locked project loop from `docs/sgpm-quickstart.md`: `sgpm metadata
   --format json --locked`, `sgpm publish --dry-run --locked --format json`,
   `sgpm check --locked`, `sgpm test --locked`, `sgpm fmt --check --locked`,
   `sgpm doc --locked`, and `sgpm build --locked`.
4. If a regression is limited to assertions or test reporting, bisect between
   `sgc` and `tools/stdlib/runtime.c` changes.

## Support matrix

Update `examples/realworld/SUPPORT_MATRIX.md` when a release changes pillar status (assertions, async, resolver, and so on).
