# sgplatform - SDL2 Platform Layer

`sgplatform` is the lower-level graphics package for the Sengoo ecosystem. It
wraps SDL2 for windowing, event polling, frame timing, and 2D drawing. Higher
packages [`sggame`](../packages/sggame/) and [`sggui`](../packages/sggui/) build
on this layer.

> **OpenSpec tasks 1.3-1.4 (done):** SDL2 version targets and supported hosts
> are recorded below; phase-1 non-goals are confirmed in Phase-1 non-goals and
> cross-linked from [GRAPHICS_SUPPORT_MATRIX.md](../packages/GRAPHICS_SUPPORT_MATRIX.md).

See also: [Graphics support matrix](../packages/GRAPHICS_SUPPORT_MATRIX.md),
[packages index](../packages/README.md).

## SDL2 Version Targets

| Target | Detail |
| --- | --- |
| API family | SDL2, not SDL3 |
| Minimum | SDL2 2.0.22+ development libraries: headers plus link library |
| Recommended / CI reference | SDL2 2.28.x or newer from the host package manager |
| Link name | `SDL2` via `#[link(name = "SDL2")]` |

Sengoo does not ship or vendor SDL2 in phase 1. Install development packages on
each host and use the environment variables below when the compiler or linker
cannot find headers or libraries automatically.

## Supported Hosts

| Host | Status |
| --- | --- |
| Windows x64 (MSVC or Clang toolchain) | Supported with SDL2 dev files and `SDL2.dll` on `PATH` at run time |
| Linux x86_64 | Supported with `libsdl2-dev` or equivalent |
| macOS | Not a phase-1 target; may work with Homebrew SDL2 but is not part of the archive gate |
| Hosts without SDL2 dev libraries | Unsupported for real native graphics builds; use documented skips for non-graphics tests |

## Linux Installation

Debian / Ubuntu:

```bash
sudo apt update
sudo apt install -y libsdl2-dev build-essential clang
pkg-config --modversion sdl2
```

Fedora / RHEL-family:

```bash
sudo dnf install SDL2-devel clang
```

If SDL2 is installed in a non-standard prefix:

```bash
export SENGOO_SDL2_INCLUDE_DIR=/opt/SDL2/include
export SENGOO_NATIVE_LIB_DIRS=/opt/SDL2/lib
export LIBRARY_PATH="/opt/SDL2/lib:${LIBRARY_PATH:-}"
export LD_LIBRARY_PATH="/opt/SDL2/lib:${LD_LIBRARY_PATH:-}"
```

## Windows Installation

You need headers, an import library (`SDL2.lib`), and the runtime DLL
(`SDL2.dll`).

vcpkg:

```powershell
vcpkg install sdl2:x64-windows
$env:SENGOO_SDL2_INCLUDE_DIR = "C:\path\to\vcpkg\installed\x64-windows\include"
$env:SENGOO_SDL2_LIB_DIR = "C:\path\to\vcpkg\installed\x64-windows\lib"
$env:PATH = "C:\path\to\vcpkg\installed\x64-windows\bin;$env:PATH"
```

Manual upstream SDL2 release:

1. Download the SDL2 development ZIP for Visual C++ or MinGW from libsdl.org.
2. Set `SENGOO_SDL2_INCLUDE_DIR` to the directory containing `SDL.h` or
   `SDL2/SDL.h`.
3. Set `SENGOO_SDL2_LIB_DIR` to the directory containing `SDL2.lib`.
4. Add the directory containing `SDL2.dll` to `PATH` before `sgc run` or before
   launching built binaries.

## Environment Variables

| Variable | Purpose |
| --- | --- |
| `SENGOO_SDL2_INCLUDE_DIR` | Extra include directory used when compiling package-native C sources |
| `SENGOO_SDL2_LIB_DIR` | Extra directory searched for `SDL2.lib` / `libSDL2` when linking |
| `SENGOO_NATIVE_LIB_DIRS` | Additional native library search directories forwarded from `#[link]` metadata |
| `SGPLATFORM_SKIP_GRAPHICS` | Compile `sgplatform_shim.c` in stub mode and drop SDL2 link metadata for non-graphics logic tests |
| `PATH` (Windows) | Must include the folder with `SDL2.dll` at run time |
| `LIBRARY_PATH` / `LD_LIBRARY_PATH` (Linux) | Optional loader paths when SDL2 is not in a default system path |
| `SDL_VIDEODRIVER` | Set to `dummy` for real headless SDL smoke runs where the dummy driver is available |

`sgc build`, `sgc run --engine native`, and `sgpm build` collect
`#[link(name = "...")]` from the root package and imported package modules and
forward names to the native linker. No new `Sengoo.toml` native-library schema is
required in phase 1.

## Package Loop

From a package directory, or with `--manifest-path` from the repository root:

```bash
sgpm update
sgpm test --locked
sgpm build --locked
sgpm doc --locked
```

For non-graphics CI or local hosts without SDL2 development files:

```bash
SGPLATFORM_SKIP_GRAPHICS=1 sgpm test --locked
```

This skip path proves package graph resolution, native C source compilation,
logic tests, and documentation. It is not a replacement for the archive gate's
required real `blank_window` smoke pass.

## Run The Blank Window Example

After SDL2 is installed and paths are set, compile and run the example through
`sgc` with a module map so `import sgplatform` resolves to the package library.

Linux / macOS shell:

```bash
cd packages/sgplatform
sgpm update
export SENGOO_MODULE_MAP="sgplatform=$(pwd)/src/lib.sg"
sgc run --engine native examples/blank_window.sg
```

Windows PowerShell:

```powershell
cd packages\sgplatform
sgpm update
$root = (Get-Location).Path
$env:SENGOO_MODULE_MAP = "sgplatform=$root/src/lib.sg"
sgc run --engine native examples\blank_window.sg
```

Expected behavior: a window opens (or a supported SDL dummy driver runs one
frame), the renderer clears and presents once, then the program exits
successfully.

## Link Failure Troubleshooting

1. Confirm SDL2 development packages are installed.
2. Confirm headers are discoverable, or set `SENGOO_SDL2_INCLUDE_DIR`.
3. Confirm libraries are discoverable, or set `SENGOO_SDL2_LIB_DIR` /
   `SENGOO_NATIVE_LIB_DIRS`.
4. On Windows, add the `SDL2.dll` directory to `PATH` even after linking
   succeeds.
5. Ensure `clang` is on `PATH`; see [debugging-native.md](debugging-native.md).
6. For headless CI skips, see
   [GRAPHICS_SUPPORT_MATRIX.md](../packages/GRAPHICS_SUPPORT_MATRIX.md).

## Phase-1 Non-Goals

Aligned with `openspec/changes/sggame-platform-gui/design.md`:

- No audio, fonts, SDL_image, or SDL_mixer.
- No static SDL2 vendoring in the repository.
- No native Win32/Cocoa/GTK widgets; `sggui` is self-drawn.
- No `adventure_shell` archive requirement.
- No promotion of graphics modules into `tools/stdlib/` before APIs stabilize.

For capability status and deferred rows, see
[packages/GRAPHICS_SUPPORT_MATRIX.md](../packages/GRAPHICS_SUPPORT_MATRIX.md).
