## Why

Sengoo can already compile native programs, run CLI tools, and exercise a
growing stdlib, but it has no first-party path for interactive graphics,
games, or desktop-style UIs. That blocks demos such as 2D games and small
utility apps, and it slows ecosystem growth because library authors cannot
dogfood windowing, input, timing, rendering, native linking, or package docs
through real packages.

A shared graphics platform with `sggame` and `sggui` on top will force
practical improvements to FFI linking, resource lifetimes, module packaging,
examples, CI, and documentation while giving users a pygame-like game API and a
lightweight GUI API without duplicating two unrelated native stacks.

## What Changes

- Introduce **`sgplatform`**, a lower-level SDL2-backed package for window
  creation, event polling, clocks, 2D rendering, keyboard/mouse input, and
  stable error/status reporting.
- Introduce **`sggame`**, a higher-level game library with pygame-inspired
  modules (`display`, `event`, `time`, `draw`, `image`, `input`) built on
  `sgplatform`.
- Introduce **`sggui`**, a lightweight self-drawn GUI package for windows,
  labels, buttons, panels, and simple layout built on the same `sgplatform`
  renderer rather than a second native UI stack.
- Add committed **`packages/`** sgpm fixtures, not `tools/stdlib/`, for the
  three libraries plus required examples (`blank_window`, `snake`, `counter`).
- Add a first-class native-link path for package FFI libraries: source
  `#[link(name = "...")]` metadata used by these packages must be forwarded to
  the native linker, and package docs must define the SDL2 library search setup.
- Add FFI/runtime/link documentation, platform dependency notes, smoke tests,
  and CI coverage for init -> frame -> quit on at least one reference host.
- Document supported, deferred, and host-specific behavior in a support matrix
  similar to `examples/realworld/SUPPORT_MATRIX.md`.

## Capabilities

### New Capabilities

- `sgplatform`: SDL2 platform layer (window, events, clock, renderer, drawing,
  input, errors, linking).
- `sggame`: pygame-style game API and examples on top of `sgplatform`.
- `sggui`: lightweight self-drawn GUI widgets and examples on top of
  `sgplatform`.

### Modified Capabilities

- `tooling-mainstream-ecosystem`: add requirements for publishing,
  documenting, linking, and testing third-party-style graphics packages through
  `sgpm` without promoting them into `tools/stdlib/` prematurely.

## Impact

- Adds new sgpm packages under `packages/sgplatform`, `packages/sggame`, and
  `packages/sggui`.
- Adds native linker forwarding for existing `#[link(name = "...")]` FFI
  metadata where required for host-native `sgc build` / `sgpm build`.
- Adds examples and integration tests in `packages/*/tests` and repository CI.
- Updates docs (`docs/sgplatform.md`, package READMEs, examples/packages index).
- Does **not** change Sengoo language syntax.
- Does **not** add a new `Sengoo.toml` manifest schema in this change.
- Does **not** vendor or statically bundle SDL2 in this change.
- Does **not** move graphics APIs into official `tools/stdlib/` in this change.

## Non-Goals

- No 3D engine, physics engine, or scene graph editor.
- No audio, fonts, SDL_image, SDL_mixer, joystick support, or shader pipeline in
  phase 1.
- No native Win32/Cocoa/GTK widget bindings in the first release.
- No multiplayer networking or game-server stack.
- No visual adventure shell in this change; keep it as a follow-up once
  `sggame` stabilizes.
- No claim of full pygame API compatibility; `sggame` targets a useful subset
  with familiar naming.
- No Electron/webview GUI.
- No requirement to support hosts without installable SDL2 development
  libraries, but archive MUST NOT rely only on platform skips.
