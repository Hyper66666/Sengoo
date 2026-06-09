# Cross-compilation with `sgc`

`sgc build` accepts an explicit `--target <triple>` flag for the reference triples
below. When `--target` is omitted, `sgc` builds for the host triple.

## Supported reference triples

| Triple | Typical host |
| --- | --- |
| `x86_64-pc-windows-msvc` | Windows x64 |
| `x86_64-unknown-linux-gnu` | Linux x64 |

Any other triple fails with a diagnostic that names the triple and points here.

## Environment variables

### `SENGOO_LINUX_SYSROOT`

Required when cross-compiling to `x86_64-unknown-linux-gnu` from a non-Linux host
(for example, Windows → Linux). Point this at a Linux sysroot tree that provides
headers and libraries for the GNU target.

```powershell
$env:SENGOO_LINUX_SYSROOT = "C:\toolchains\linux-gnu\x86_64-unknown-linux-gnu"
sgc build src\main.sg --target x86_64-unknown-linux-gnu
```

```bash
export SENGOO_LINUX_SYSROOT=/opt/toolchains/x86_64-unknown-linux-gnu
sgc build src/main.sg --target x86_64-unknown-linux-gnu
```

`sgc` passes `--sysroot` to Clang for object compilation and uses Clang with
`-fuse-ld=lld` for cross links. A missing or incomplete sysroot surfaces as a
linker error with remediation pointing back to this document.

### `SENGOO_WINDOWS_SDK_ROOT`

Required when cross-compiling to `x86_64-pc-windows-msvc` from a non-Windows host.
Point this at a Windows SDK include root containing `ucrt`, `um`, and `shared`.

```bash
export SENGOO_WINDOWS_SDK_ROOT=/opt/windows-sdk/Include/10.0.22621.0
sgc build src/main.sg --target x86_64-pc-windows-msvc
```

## Host-native builds

When `--target` matches the host triple, `sgc` uses the existing native toolchain
policy:

- Windows hosts link with MSVC `link.exe` and bundled Windows/MSVC include paths.
- Linux hosts link with Clang (lld when available).

## Examples

Build for the host (default):

```bash
sgc build src/main.sg
```

Build for the other reference triple on a Windows host (requires sysroot):

```powershell
sgc build src\main.sg --target x86_64-unknown-linux-gnu
```

Emit compile timings alongside a cross build:

```bash
sgc build src/main.sg --target x86_64-unknown-linux-gnu --timings-json target/timings.json
```

## Limitations (v1)

- Only the two reference triples above are accepted.
- Cross builds require Clang on `PATH`.
- macOS hosts are not a documented reference pair in v1; use a Linux or Windows
  reference host for cross-compile validation.
