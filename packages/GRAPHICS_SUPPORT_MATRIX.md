# Graphics Support Matrix

User-facing fact source for graphics capabilities in the `sggame-platform-gui`
change. Phase 1 targets **Windows** and **Linux** hosts with installable SDL2
development libraries. Packages live under `packages/` as sgpm fixtures (not
`tools/stdlib/`).

> **OpenSpec task 1.2 (done):** this file is the authoritative capability matrix
> with columns Capability, Status, Host scope, Proof example/test, Stable
> diagnostic/status, and Upstream package/change.

| Capability | Status | Host scope | Proof example/test | Stable diagnostic/status | Upstream package/change |
| --- | --- | --- | --- | --- | --- |
| Window creation and 2D renderer | supported | Windows x64, Linux x86_64 with SDL2 dev libs | `packages/sgplatform/examples/blank_window.sg`; `packages/sgplatform/tests/` smoke (`init -> pump -> present -> quit`) | `STATUS_PLATFORM` on SDL init failure; `STATUS_INVALID_HANDLE` on closed handles; link failures name SDL2 and point to `docs/sgplatform.md` | `sgplatform`, `sggame-platform-gui` |
| Keyboard and mouse input (event poll) | supported | Same as windowing | `packages/sggame/examples/snake.sg`; `packages/sggui/tests/` constructed `Event` / hit-test paths | `EVENT_NONE`, `EVENT_QUIT`, key/mouse event kinds; invalid poll on destroyed platform returns stable status | `sgplatform`, `sggame`, `sggui` |
| 2D drawing (clear, rect, line) | supported | Same as windowing | `blank_window` clear/present; `snake` rectangle draw; `sggui` counter self-drawn widgets | `STATUS_INVALID_ARGUMENT` for bad color/rect args; `STATUS_INVALID_HANDLE` on destroyed renderer | `sgplatform`, `sggame`, `sggui` |
| Audio (SDL_mixer / sound) | deferred | N/A in phase 1 | Matrix row only; no success claim | `STATUS_UNSUPPORTED` if called before a future audio change | Follow-up OpenSpec (not `sggame-platform-gui`) |
| Font rendering (TTF / text layout) | deferred | N/A in phase 1 | Matrix row only; labels use fixed string drawing without font stack | `STATUS_UNSUPPORTED` for font APIs not shipped in phase 1 | Follow-up OpenSpec |
| Image loading (SDL_image) | deferred | N/A in phase 1 | `sggame.image.load` may return `STATUS_UNSUPPORTED`; snake uses rectangles only | `STATUS_UNSUPPORTED` from `sggame.image.load` | `sggame` documents fallback; SDL_image is a follow-up |
| Native OS GUI widgets (Win32/Cocoa/GTK) | deferred | N/A in phase 1 | `sggui` is self-drawn on `sgplatform` renderer, not system controls | `STATUS_UNSUPPORTED` for native widget APIs | `sggui` README; `sggame-platform-gui` design non-goals |
| Static SDL2 vendoring / bundling | deferred | N/A in phase 1 | Users install system or package-manager SDL2; no vendored copy in repo | Link/setup docs in `docs/sgplatform.md` | `sggame-platform-gui` design resolved questions |
| CI headless graphics smoke skip | host-skip | CI agents without dummy video driver or SDL2 | `SDL_VIDEODRIVER=dummy` where supported; otherwise skip recorded here with reason | Skip reason: missing SDL2, dummy driver unavailable, or headless host | `sggame-platform-gui` design section 10; archive gate requires at least one non-skipped `blank_window` pass |

## Phase-1 Scope

These items are **out of scope** for the initial `sggame-platform-gui` archive and
are documented here rather than implied by package READMEs:

- No **SDL_image**, **SDL_mixer**, or font stack in phase 1.
- No **static vendoring** of SDL2; use host dev packages and documented env vars.
- No **native OS widgets**; `sggui` draws with the shared SDL2 renderer.
- No **`adventure_shell`** example as an archive gate (follow-up change only).
- Graphics APIs stay in **`packages/`** until stable; not promoted to `tools/stdlib/`.

## Additional Deferred Capabilities

| Capability | Status | Notes |
| --- | --- | --- |
| GPU shaders / 3D | deferred | 2D renderer only in phase 1 |
| Joystick / gamepad | deferred | Keyboard/mouse subset only |
| High-DPI / multi-monitor policy | deferred | Single window, default DPI |
| Async / reactor frame loop | deferred | Synchronous game loop first |

## Archive Gate

The change MUST NOT archive if **every** host row records a graphics smoke skip.
At least one reference host MUST list a passing `blank_window` (or equivalent)
smoke proof before archive.

Reference host evidence:

- 2026-06-08, Windows x64 local host, temporary SDL2 VC development package
  `SDL2-devel-2.32.8-VC.zip`: `sgpm test --locked` passed for `sgplatform`,
  `sggame`, and `sggui` without `SGPLATFORM_SKIP_GRAPHICS`; `blank_window`,
  `snake`, and `counter` examples passed `sgc run --engine native --force-rebuild`
  with exit code 0.
