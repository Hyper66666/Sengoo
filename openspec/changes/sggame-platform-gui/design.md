## Context

Sengoo today provides terminal IO (`std::io`), file/json/http/process stdlib
modules, FFI to C, and sgpm packaging, but no windowing or rendering stack.
Users asked for ecosystem libraries comparable to pygame (games) and a simple
GUI toolkit. Building both on separate native stacks would duplicate event
loops, input handling, rendering, and linking pain.

This change introduces three **sgpm packages** that share one SDL2-backed
platform layer. Game and GUI libraries become consumers of `sgplatform`, not
parallel FFI islands.

Stakeholders:

- Application authors wanting 2D games and small desktop utilities.
- Toolchain maintainers needing real packages to stress FFI, linking, docs, and
  CI.
- Language/runtime teams receiving feedback on structs, methods, modules,
  `Result`, and resource cleanup patterns.

## Goals / Non-Goals

**Goals:**

- Ship `sgplatform`, `sggame`, and `sggui` as independent sgpm packages with
  README, tests, and at least one runnable example each.
- Provide a stable init -> poll events -> render -> present -> quit loop on
  Windows and Linux hosts with SDL2 development libraries installed.
- Expose pygame-familiar `sggame` module names for the supported subset.
- Expose minimal `sggui` widgets (app/window, label, button, panel/layout).
- Make host-native SDL2 linking reproducible through `sgc` / `sgpm`.
- Document SDL2 install/link steps and unsupported host behavior.
- Drive ecosystem improvements: package dependencies, `sgc doc`, smoke tests,
  support matrix.

**Non-Goals:**

- Full pygame parity, 3D, tilemap editor, asset pipeline, audio, font rendering,
  SDL_image, or SDL_mixer in phase 1.
- System-native controls (Win32/Cocoa/GTK) in phase 1.
- Async/reactor integration for frame loops in phase 1; use a synchronous game
  loop first.
- New `Sengoo.toml` manifest schema for native libraries in this change.
- Static vendoring/bundling of SDL2 in this change.
- Promoting graphics modules into `tools/stdlib/` before package APIs stabilize.

## Decisions

### 1. Three-package layering

```text
Application / examples
        |
   sggame   sggui
        \   /
      sgplatform
          |
        SDL2
```

**Rationale:** One event pump, one renderer, one linking story.

**Rejected:** Monolithic `sggame` with optional GUI submodule, because GUI users
should not depend on game-specific APIs.

### 2. Native backend: SDL2

**Rationale:** Cross-platform window + 2D renderer + input + timing in one
dependency; sufficient for MVP games and self-drawn GUI.

**Rejected:** Raylib (less GUI flexibility), pure software framebuffer (dead
end), wgpu/GL (too heavy for phase 1).

### 3. Package location: `packages/`, not `tools/stdlib/`

Paths:

```text
packages/sgplatform/
packages/sggame/
packages/sggui/
```

Examples live inside each package, for example:

```text
packages/sgplatform/examples/blank_window.sg
packages/sggame/examples/snake.sg
packages/sggui/examples/counter.sg
```

**Rationale:** Exercises sgpm dependencies, versioning, and third-party author
workflow. Official stdlib promotion waits for API stability.

### 4. Native link contract

Phase 1 uses Sengoo's existing source-level `#[link(name = "...")]` FFI
metadata. The implementation MUST ensure host-native builds collect link names
from the root package and imported package modules, then forward them to the
native linker.

Minimum contract:

- `sgplatform` declares SDL2 externs with `#[link(name = "SDL2")]` or a
  documented host-specific equivalent.
- `sgc build`, `sgc run --engine native`, and `sgpm build` include those link
  libraries when linking the final executable.
- Linux uses system linker lookup plus documented `LIBRARY_PATH` /
  `SENGOO_NATIVE_LIB_DIRS` style setup if an extra search directory is needed.
- Windows uses MSVC/Clang-compatible search paths plus documented
  `SENGOO_SDL2_LIB_DIR` and `PATH` setup for `SDL2.dll`.
- The change MAY add narrow native-linker plumbing to `sgc`, but it MUST NOT add
  a new package manifest schema. If manifest-level native libraries become
  necessary, open a follow-up change.
- Link failures MUST produce diagnostics that name the missing SDL2 library and
  point to `docs/sgplatform.md`.

### 5. FFI style matches existing stdlib patterns

- `extern "C"` bindings in `.sg` files are preferred.
- If a small C shim is needed for portability, it belongs to `packages/sgplatform/native/`
  and must be compiled/linked by documented package commands.
- Resource handles are wrapped in structs with explicit `close()` / `destroy()`
  methods.
- Errors use `Result<_, i64>` and stable status constants; no panics on
  user-facing paths.
- Invalid or already-destroyed handles return stable errors.

### 6. `sgplatform` minimal API

Phase 1 freezes these public names unless implementation discovers a blocking
compiler limitation and updates the spec before coding around it.

| Type / module | Required API |
|---------------|--------------|
| `sgplatform` | `init() -> Result<Platform, i64>`, `quit(platform)` |
| `Platform` | `create_window(title, width, height) -> Result<Window, i64>`, `poll_event() -> Event`, `ticks_ms() -> i64`, `delay_ms(ms)` |
| `Window` | `create_renderer() -> Result<Renderer, i64>`, `close()` |
| `Renderer` | `clear(color)`, `present()`, `draw_rect(rect, color)`, `fill_rect(rect, color)`, `draw_line(x1, y1, x2, y2, color)`, `destroy()` |
| `Rect` | `x`, `y`, `w`, `h` integer fields |
| `Color` | `r`, `g`, `b`, `a` integer fields |
| `Event` | `kind`, `key`, `mouse_x`, `mouse_y`, `mouse_button` integer fields |

Required event/status constants:

- Event kinds: `EVENT_NONE`, `EVENT_QUIT`, `EVENT_KEY_DOWN`,
  `EVENT_KEY_UP`, `EVENT_MOUSE_BUTTON_DOWN`, `EVENT_MOUSE_BUTTON_UP`.
- Keys: at least `KEY_LEFT`, `KEY_RIGHT`, `KEY_UP`, `KEY_DOWN`, `KEY_W`,
  `KEY_A`, `KEY_S`, `KEY_D`, `KEY_ESCAPE`.
- Status: `STATUS_OK`, `STATUS_INVALID_ARGUMENT`, `STATUS_INVALID_HANDLE`,
  `STATUS_UNSUPPORTED`, `STATUS_IO`, `STATUS_PLATFORM`.

### 7. `sggame` API shape (pygame-inspired subset)

Modules:

| Module | Responsibility |
|--------|----------------|
| `sggame` | `sggame_init()`, `sggame_quit()`, top-level helpers |
| `sggame.display` | `set_mode(width, height, title)`, `flip()`, `get_surface()` |
| `sggame.event` | `poll()`, `pump()`, event/key constants re-exported from `sgplatform` |
| `sggame.time` | `Clock`, `Clock.tick(fps)` returning elapsed milliseconds |
| `sggame.draw` | `rect(color, rect)`, `line(color, x1, y1, x2, y2)` |
| `sggame.image` | `load(path) -> Result<Texture, i64>` MAY return `STATUS_UNSUPPORTED` in phase 1; no SDL_image dependency |
| `sggame.input` | `key_pressed(key)`, mouse position helpers where SDL state is available |

The required `snake` example MUST use only supported phase-1 APIs. If image
loading is unsupported, snake uses rectangles.

### 8. `sggui` API shape (phase 1 self-drawn)

Phase 1 names:

- `sggui.App`: title, width, height, `begin_frame()`, `end_frame()`, `poll()`,
  `close()`.
- `sggui.Label`: text/value plus bounds.
- `sggui.Button`: text/value plus bounds, `hit_test(x, y) -> bool`,
  `handle_event(event) -> bool`.
- `sggui.Panel`: simple vertical grouping with fixed spacing.

Rendering uses `sgplatform` renderer, not OS widgets.

**Future:** optional `sggui-imgui` change for Dear ImGui; out of scope here.

### 9. Examples and proof apps

| Example | Package | Required | Proves |
|---------|---------|----------|--------|
| `blank_window` | sgplatform | yes | init/present/quit/linking |
| `snake` | sggame | yes | input, clock, draw, collision |
| `counter` | sggui | yes | button + label + event routing |
| `adventure_shell` | sggame | no | follow-up only |

### 10. Testing and skip policy

- **Reference smoke:** at least one supported host MUST run a real
  `blank_window` init -> pump -> present -> quit smoke before archive.
- **Headless CI:** use `SDL_VIDEODRIVER=dummy` where supported. If dummy mode is
  unavailable, CI may skip graphics smoke only after recording the skip in
  `packages/GRAPHICS_SUPPORT_MATRIX.md`.
- **No all-skip archive:** the change MUST NOT archive if every host records a
  graphics smoke skip.
- **Logic tests:** snake movement/collision and button hit-test run without
  opening a visible window.
- **Event injection:** tests for `sggui.Button` use pure hit-testing and/or
  constructed `Event` values; manual clicking is not the only proof.
- **Manual/demo:** README commands document local visual verification for
  blank window, snake, and counter.
- **sgpm:** each package supports `sgpm update`, `sgpm test --locked`, and
  `sgpm build --locked` after SDL2 is available.

### 11. Documentation and support matrix

Add `packages/GRAPHICS_SUPPORT_MATRIX.md` with columns:

- Capability
- Status (`supported`, `deferred`, `host-skip`, `unsupported`)
- Host scope
- Proof test/example
- Stable diagnostic
- Upstream/native dependency

Initial deferred rows: audio, font rendering, native widgets, GPU shaders,
joystick, high-DPI policy, SDL_image, static SDL2 vendoring, async frame loop.

## Risks / Trade-offs

| Risk | Mitigation |
|------|------------|
| SDL2 install friction | Document apt/brew/vcpkg/manual Windows setup; CI installs dev package where available |
| Headless CI cannot open windows | Smoke test `SDL_VIDEODRIVER=dummy`; require at least one real reference smoke before archive |
| `#[link]` metadata is not currently enough for native linker | Make native linker forwarding a blocking sgplatform task |
| Handle-heavy APIs feel unidiomatic | Wrap in structs + methods; keep names stable in phase 1 |
| `sggame` vs pygame API drift | Document subset; avoid claiming compatibility |
| GUI expectations too high | Call phase 1 "self-drawn lightweight GUI" |
| Blocking game loop vs async roadmap | Sync loop first; note future reactor integration in matrix |

## Migration Plan

1. Land native-link forwarding and diagnostics needed by `#[link(name = "SDL2")]`.
2. Land `sgplatform` with blank window example and link docs.
3. Land `sggame` display/event/time/draw + snake.
4. Land `sggui` counter example.
5. Wire CI job `graphics-smoke` or extend `realworld-e2e` for package tests.
6. Update repository docs with graphics package entry points.
7. Archive change after strict validation and green verification tasks.

Rollback: packages are additive; removing them does not break compiler/stdlib.

## Resolved Questions

- Static SDL2 vendoring: out of scope for this change.
- SDL_image/image loading: no SDL_image in phase 1; `sggame.image.load` may be
  unsupported but documented.
- Example package naming: examples live inside each package.
- Adventure shell: follow-up change, not part of this archive gate.
