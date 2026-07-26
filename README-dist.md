# Sengoo Toolchain Archive

This archive contains the Sengoo command-line tools:

- `bin/sgc`
- `bin/sgpm`
- `bin/sgfmt`
- `bin/sglsp`

It also includes the standard library, C runtime bridge, and target-native
runtime under `share/sengoo/`, so `sgc` can resolve `std::*` imports and link
native programs without a source checkout, `SENGOO_ROOT`, or Cargo.

## Installed Runtime Layout

The archive root uses manifest schema 2:

```text
manifest.json
payloads.sha256
bin/sgc[.exe]
share/sengoo/stdlib/runtime.c
share/sengoo/stdlib/runtime_*.c
share/sengoo/stdlib/runtime_shared.h
share/sengoo/runtime/<target>/sengoo_runtime.lib     # Windows
share/sengoo/runtime/<target>/libsengoo_runtime.a    # Unix
```

`manifest.json.native_runtime` binds runtime ABI 1 to the exact target,
relative library path, SHA-256, ordered platform link arguments, and declared
dynamic dependencies. `build_manifest_id` is the SHA-256 of the normalized
`payloads.sha256` file. Installers verify the archive checksum, every listed
payload, and that no unlisted payload file is present before copying files.

`source_revision`, `source_dirty`, `artifact_provenance`, and
`release_eligible` distinguish clean packager-built archives from local
`-NoBuild` development bundles. A dirty or `-NoBuild` bundle is useful for
local smoke testing but is not release or Senline pin evidence.

Installed native build/run validates the manifest and runtime before cache
reuse. Missing, relocated-without-manifest, wrong-target, wrong-ABI, tampered,
or incomplete installations fail without consulting Cargo or a compiled-in
source checkout.

## Reproducible Distribution Gate

Windows x64 and Linux x64 CI package the same clean Git revision twice in
sequence. Build A and build B use separate empty Cargo target directories.
Each target directory is remapped to the common virtual
`/sengoo-build/target` prefix so runner-local build paths cannot affect shipped
payloads. Both manifests must report `source_dirty=false` and
`release_eligible=true`, and both checksum-verifying installers must accept the
resulting archives.

`scripts/compare-distribution-manifests.ps1` validates the complete schema 2
shape before comparing it. Normalization is deliberately narrow:

- `tools`, `stdlib_modules`, `runtime_sources`, and
  `native_runtime.dynamic_dependencies` are compared as sorted unique sets.
- `tool_versions` is ordered by its exact tool keys.
- `payloads` is ordered by normalized relative path; paths must also be unique
  under case-insensitive comparison.
- `native_runtime.link_args` retains order because linker argument order is
  part of the installed runtime contract.

Only `generated_at_utc`, `runner_os`, `runner_image`, and the run-specific
`smoke_evidence` provenance note are excluded. GitHub provenance attestation
signatures are external to manifest schema 2 and may differ without weakening
the payload comparison. `artifact_provenance`, release eligibility, source
revision and dirtiness, build identity, tool versions, payload sizes and
hashes, runtime ABI/library/hash, ordered link arguments, dynamic dependency
identities, archive/checksum names, and license presence must match exactly.
Unknown or missing fields fail validation rather than disappearing during
normalization.

The gate retains `normalized-a.json`, `normalized-b.json`, and
`comparison.json`, including both normalized SHA-256 values and any excluded
provenance differences. A mismatch blocks the target before publication.

## Source Runtime Development Mode

`sgc` defaults to `--runtime-mode installed`. Contributors who intentionally
build the native Rust runtime through Cargo must use a compiler executable
inside the Sengoo workspace and opt in explicitly:

```sh
target/debug/sgc --runtime-mode source-development build path/to/main.sg
```

Locked package loops use the same explicit policy at the `sgpm` boundary; the
selected mode is forwarded to every delegated `sgc` command:

```sh
target/debug/sgpm --runtime-mode source-development check --locked
target/debug/sgpm --runtime-mode source-development test --locked
```

This mode emits the stable `toolchain::source_runtime_development` diagnostic
and records `artifact_provenance=source-cargo-development`,
`release_eligible=false`, and `senline_pin_evidence=false` in build/run cache
metadata. It cannot start or dispatch through the compiler daemon. Moving the
compiler outside its source workspace does not carry this authority with it.
Artifacts produced in this mode are development outputs only and are never
installed-distribution, publication, or Senline pin evidence.

## Requirements

Native builds require LLVM/Clang 15 or newer on `PATH`. Windows builds also
require the supported MSVC linker/SDK. `sgc build --emit-llvm` and stdlib
import expansion work without native linking.

## Quickstart

```sh
bin/sgc --version
cat > hello.sg <<'EOF'
import std::status;

def main() -> i64 {
    STATUS_OK()
}
EOF
bin/sgc build hello.sg --emit-llvm
```

For installed use, add the archive's `bin` directory to `PATH`.

PowerShell:

```powershell
.\scripts\install.ps1 -Archive .\sengoo-<version>-<target>.zip
.\scripts\install.ps1 -Version <version>
```

POSIX shell:

```sh
scripts/install.sh ./sengoo-<version>-<target>.tar.gz
scripts/install.sh --version <version>
```

Published channels use these target names:

- `x86_64-pc-windows-msvc`
- `x86_64-unknown-linux-gnu`
- `aarch64-apple-darwin`
- `x86_64-apple-darwin`

The installers detect the current host target. Pass `--target` / `-Target` to
override it, or `--print-target` / `-PrintTarget` to inspect the selected
channel. Re-running the version command with a newer pinned version performs an
explicit checksum-verified upgrade; the scripts never auto-update.

Release archives also carry GitHub build-provenance attestations. After
downloading from GitHub Releases, verify one with:

```sh
gh attestation verify sengoo-<version>-<target>.tar.gz --repo Hyper66666/Sengoo
```
