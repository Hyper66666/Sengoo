## ADDED Requirements

### Requirement: sgplatform SHALL use reproducible native SDL2 linking

The `sgplatform` package SHALL declare SDL2 FFI through source-level
`#[link(name = "SDL2")]` or a documented host-specific equivalent, and
host-native `sgc` / `sgpm` builds SHALL forward that metadata to the final
native linker.

#### Scenario: Link metadata reaches the native linker

- **WHEN** a package imports `sgplatform` and runs `sgpm build` on a host with
  SDL2 development libraries installed
- **THEN** the final executable links SDL2 without requiring the user to edit
  generated LLVM IR or object files
- **AND** the build documents any required library search path environment such
  as `SENGOO_SDL2_LIB_DIR`

#### Scenario: Missing SDL2 is diagnosable

- **WHEN** SDL2 headers or libraries are unavailable during a native build
- **THEN** the build reports a stable diagnostic naming SDL2 and linking to
  `docs/sgplatform.md`
- **AND** the failure occurs before an opaque unresolved-symbol crash at runtime

### Requirement: sgplatform SHALL provide SDL2-backed window and renderer initialization

The `sgplatform` package SHALL expose a documented API to initialize SDL2, create
a window, create a 2D renderer, and shut down resources in reverse order without
leaking handles across a normal quit path.

#### Scenario: Blank window smoke path

- **WHEN** a program calls the documented `sgplatform` init API, opens a window
  with a requested width and height, clears the renderer, presents one frame, and
  quits
- **THEN** the program exits with success on supported hosts
- **AND** all created platform resources are released on quit
- **AND** failures return stable error/status results instead of aborting the
  process on user-facing paths

#### Scenario: Public initialization names are stable

- **WHEN** a user follows the `sgplatform` README
- **THEN** the documented phase-1 names include `sgplatform.init`,
  `Platform.create_window`, `Window.create_renderer`, `Renderer.clear`,
  `Renderer.present`, `Renderer.destroy`, and `Window.close`
- **AND** code examples use those names rather than private FFI bridge symbols

#### Scenario: Unsupported host reports stable failure

- **WHEN** SDL2 initialization fails because the host lacks graphics support or
  development libraries
- **THEN** `sgplatform` returns a stable error/status code and message channel
- **AND** documentation lists the required native dependency and accepted CI skip
  behavior

### Requirement: sgplatform SHALL expose event polling and frame timing

The `sgplatform` package SHALL provide event polling for quit and keyboard/mouse
input plus a monotonic clock or tick helper suitable for game and GUI loops.

Event values SHALL expose at least integer fields `kind`, `key`, `mouse_x`,
`mouse_y`, and `mouse_button`. The package SHALL document constants for
`EVENT_NONE`, `EVENT_QUIT`, `EVENT_KEY_DOWN`, `EVENT_KEY_UP`,
`EVENT_MOUSE_BUTTON_DOWN`, `EVENT_MOUSE_BUTTON_UP`, arrow keys, WASD keys, and
Escape.

#### Scenario: Quit event ends the loop

- **WHEN** the user or program posts a quit/close event
- **THEN** the event pump reports quit to the caller
- **AND** the caller can exit the main loop without undefined behavior

#### Scenario: Frame timing is available

- **WHEN** a loop calls the documented clock/tick helper with a target frame
  rate
- **THEN** the helper sleeps or waits so successive frames do not busy-spin
  unconditionally
- **AND** elapsed time is available to callers in documented units

### Requirement: sgplatform SHALL expose 2D drawing primitives on a renderer

The `sgplatform` package SHALL support clearing the framebuffer and drawing
filled rectangles and lines with documented color parameters on the active
renderer.

The phase-1 public data types SHALL include `Rect { x, y, w, h }` and
`Color { r, g, b, a }` with integer fields.

#### Scenario: Clear and draw one frame

- **WHEN** a caller clears the renderer to a solid color and draws a rectangle
- **THEN** the presented frame reflects the draw commands on supported hosts
- **AND** invalid renderer or color arguments return stable errors

### Requirement: sgplatform smoke coverage SHALL NOT archive as all-skip

The graphics smoke test MAY record documented host skips, but the change SHALL
NOT be archived unless at least one supported reference host runs a real
`sgplatform` init -> pump -> present -> quit smoke.

#### Scenario: Reference host proves graphics smoke

- **WHEN** archive review checks `packages/GRAPHICS_SUPPORT_MATRIX.md`
- **THEN** at least one supported host row lists a passing `blank_window` or
  equivalent smoke proof
- **AND** rows that skip must include a stable reason such as missing SDL2 or
  unavailable video driver

### Requirement: sgplatform SHALL document linking and packaging requirements

The `sgplatform` package SHALL ship README and docs that state SDL2 development
library requirements, supported hosts, link flags or toolchain expectations,
and how `sgpm build` produces a runnable binary.

#### Scenario: Package README explains native dependencies

- **WHEN** a user opens the `sgplatform` package documentation
- **THEN** they can find install commands for SDL2 on supported platforms
- **AND** they can find the smoke example command to verify setup
