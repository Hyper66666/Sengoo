# sggame

Pygame-inspired 2D game API on [`sgplatform`](../sgplatform/). Phase 1 covers a
synchronous `init -> poll -> draw -> flip -> quit` loop with keyboard input and
frame timing. Audio, fonts, and image loading are out of scope.

## Requirements

- Sengoo toolchain (`sgc`, `sgpm`) from this repository
- [`sgplatform`](../sgplatform/) with SDL2 development libraries for real graphics
- Native linker configured per [docs/sgplatform.md](../../docs/sgplatform.md)

## Build And Test

From this package directory:

```bash
sgpm update
sgpm test --locked
sgpm build --locked
sgpm doc --locked
```

On hosts without SDL2 development files:

```bash
SGPLATFORM_SKIP_GRAPHICS=1 sgpm test --locked
```

`tests/init_smoke.sg` locks the public lifecycle entry points. `tests/snake_logic.sg`
covers movement, wall collision, food overlap, and growth without opening a
window.

## Run The Snake Example

`examples/snake.sg` needs SDL2 installed and a module map:

```bash
export SENGOO_MODULE_MAP="sggame=$(pwd)/src/lib.sg;sgplatform=$(pwd)/../sgplatform/src/lib.sg"
sgc run --engine native examples/snake.sg
```

Windows PowerShell:

```powershell
$root = (Get-Location).Path
$plat = Join-Path (Split-Path $root -Parent) "sgplatform/src/lib.sg"
$env:SENGOO_MODULE_MAP = "sggame=$root/src/lib.sg;sgplatform=$plat"
sgc run --engine native examples\snake.sg
```

Controls: arrow keys or WASD to steer; reach the red food square or hit a wall
to end; close the window or press Escape to quit.

## Public API

Because Sengoo package imports currently expose a flat symbol table, the root
game lifecycle functions use prefixed names to avoid colliding with
`sgplatform.init()` / `sgplatform.quit(platform)`.

| Module | Entry points |
| --- | --- |
| `sggame` | `sggame_init()`, `sggame_quit()` |
| `sggame.display` | `set_mode(width, height, title)`, `flip()`, `get_surface()` |
| `sggame.event` | `poll()`, `pump()`; event/key constants from `sgplatform` |
| `sggame.time` | `Clock`, `clock_new()`, `clock_tick(clock, fps)`, `Clock.tick(fps)` |
| `sggame.draw` | `clear(color)`, `rect(color, rect)`, `line(color, x1, y1, x2, y2)` |
| `sggame.image` | `load(path)` returns `STATUS_UNSUPPORTED` in phase 1 |
| `sggame.input` | `key_pressed(key)`, `mouse_x()`, `mouse_y()` |

Errors use `Result<_, i64>` with `STATUS_*` constants from `sgplatform` /
`std::status`.

## Limitations

- Not pygame-compatible; this is a documented phase-1 subset.
- Global display/input state lives in `native/sggame_state.c`.
- No audio, fonts, textures, PNG/JPEG loading, or tilemaps in phase 1.
