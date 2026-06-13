## ADDED Requirements

### Requirement: sggame SHALL provide a pygame-inspired game API on sgplatform

The `sggame` package SHALL depend on `sgplatform` and expose familiar module
names for the supported subset: top-level `sggame_init`/`sggame_quit`,
`display`, `event`, `time`, `draw`, `image`, and `input`. It SHALL NOT
re-export a second native graphics stack.

Because Sengoo package imports currently expose a flat symbol table, the
top-level game lifecycle functions use the prefixed names `sggame_init` and
`sggame_quit` to avoid colliding with `sgplatform.init` / `sgplatform.quit`
symbols brought in through the package dependency.

The phase-1 public names SHALL include `sggame_init`, `sggame_quit`,
`sggame.display.set_mode`, `sggame.display.flip`, `sggame.event.poll`,
`sggame.event.pump`, `sggame.time.Clock.tick`, `sggame.draw.rect`,
`sggame.draw.line`, `sggame.image.load`, and `sggame.input.key_pressed`.
`sggame.image.load` MAY return `STATUS_UNSUPPORTED` in phase 1 and SHALL NOT
require SDL_image.

#### Scenario: Minimal game loop compiles and runs

- **WHEN** a user writes a program that calls `sggame_init()`, opens a display
  with `sggame.display.set_mode`, runs a loop that pumps events, draws, and
  ticks a clock, then calls `sggame_quit()`
- **THEN** the program builds through `sgpm build` on supported hosts
- **AND** the window opens and closes cleanly using only `sggame` public APIs

#### Scenario: Public modules are documented

- **WHEN** a user runs `sgpm doc` for the `sggame` package
- **THEN** the generated docs list the supported modules and entry points
- **AND** deferred pygame features are not implied as supported without tests

#### Scenario: Image loading is explicitly phase-1 limited

- **WHEN** a user calls `sggame.image.load` on a phase-1 build without SDL_image
- **THEN** the call returns a documented `STATUS_UNSUPPORTED` result or a
  documented rectangle/texture-only fallback
- **AND** `sggame` documentation does not imply general PNG/JPEG support

### Requirement: sggame SHALL ship a snake reference example

The `sggame` package or its examples directory SHALL include a snake game that
uses keyboard input, frame timing, drawing, and quit handling.

#### Scenario: Snake example is discoverable

- **WHEN** a user follows the `sggame` README quickstart
- **THEN** they can build and run the snake example with documented commands
- **AND** the example uses arrow-key or WASD input and ends on quit

#### Scenario: Snake logic has automated coverage

- **WHEN** CI runs `sgpm test` for the snake package or example tests
- **THEN** at least movement/collision logic is covered without requiring a
  human to play the game
- **AND** graphics smoke tests can skip on headless hosts with documented reason

### Requirement: sggame SHALL defer adventure shell examples

The visual adventure shell SHALL NOT be required for this change. If an
adventure shell is added early, it SHALL remain outside the archive gate unless
its logic and rendering tests are explicitly added to this change.

#### Scenario: Adventure shell is not an implicit blocker

- **WHEN** archive review checks `sggame` examples
- **THEN** `snake` is the required game proof
- **AND** absence of `adventure_shell` does not block archive
