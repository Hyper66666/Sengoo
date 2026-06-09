# sgplatform

SDL2-backed windowing, input polling, frame timing, and 2D drawing for Sengoo
games (`sggame`) and self-drawn GUI (`sggui`).

Phase 1 provides a synchronous `init -> poll -> render -> present -> quit` loop.
Audio, fonts, SDL_image, and static SDL2 vendoring are out of scope.

## Requirements

- Sengoo toolchain (`sgc`, `sgpm`) from this repository
- SDL2 development libraries on hosts that run real graphics
- Native linker (`clang` plus the platform linker, or MSVC `link.exe`)

See [docs/sgplatform.md](../../docs/sgplatform.md) for SDL2 installation,
`SENGOO_SDL2_INCLUDE_DIR`, `SENGOO_SDL2_LIB_DIR`, and runtime DLL/search-path
setup.

## Build And Test

From this package directory:

```bash
sgpm update
sgpm test --locked
sgpm build --locked
sgpm doc --locked
```

On hosts without SDL2 development files, compile and test the package graph in
stub mode:

```bash
SGPLATFORM_SKIP_GRAPHICS=1 sgpm test --locked
```

`sgpm` compiles C sources under `native/` and forwards SDL2 link metadata from
`#[link(name = "SDL2")]`. No `Sengoo.toml` native-link schema is required.

## Run The Blank Window Example

`examples/blank_window.sg` is a real graphics smoke. It needs SDL2 installed and
a module map so `import sgplatform` resolves to this package:

```bash
export SENGOO_MODULE_MAP="sgplatform=$(pwd)/src/lib.sg"
sgc run --engine native examples/blank_window.sg
```

Windows PowerShell:

```powershell
$root = (Get-Location).Path
$env:SENGOO_MODULE_MAP = "sgplatform=$root/src/lib.sg"
sgc run --engine native examples\blank_window.sg
```

Expected behavior: one window opens, clears, presents once, then exits cleanly.

## Public API

| Symbol | Role |
| --- | --- |
| `init()` / `quit(platform)` | SDL2 init and shutdown |
| `Platform` | `create_window`, `poll_event`, `ticks_ms`, `delay_ms` |
| `Window` | `create_renderer`, `close` |
| `Renderer` | `clear`, `present`, `draw_rect`, `fill_rect`, `draw_line`, `destroy` |
| `Rect`, `Color`, `Event` | Data types with integer fields |
| `EVENT_*`, `KEY_*`, `STATUS_*` | Constants |

Errors use `Result<_, i64>` with `STATUS_OK`, `STATUS_INVALID_ARGUMENT`,
`STATUS_INVALID_HANDLE`, `STATUS_UNSUPPORTED`, `STATUS_IO`, and
`STATUS_PLATFORM`.

## Limitations

- Real graphics builds require host SDL2 development files.
- `SGPLATFORM_SKIP_GRAPHICS=1` is for logic/package smoke only; it is not archive
  evidence for a real window pass.
- No audio, fonts, textures, or OS-native widgets in phase 1.
