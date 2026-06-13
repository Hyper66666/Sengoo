# sggui Specification

## Purpose
Define the lightweight self-drawn GUI package built on `sgplatform`, including
the phase-1 app loop, label, button, panel, and counter-demo expectations.
## Requirements
### Requirement: sggui SHALL provide a lightweight self-drawn GUI on sgplatform

The `sggui` package SHALL depend on `sgplatform` and expose a minimal widget set
for small desktop-style utilities: application/window loop, label, button, and
simple layout/grouping. Widgets SHALL be drawn through the shared renderer, not
through OS-native control APIs in this change.

The phase-1 public names SHALL include `sggui.App`, `App.begin_frame`,
`App.end_frame`, `App.poll`, `App.close`, `sggui.Label`, `sggui.Button`,
`Button.hit_test`, `Button.handle_event`, and `sggui.Panel`.

#### Scenario: Counter example runs

- **WHEN** a user builds and runs the documented counter example
- **THEN** a window displays a label and button
- **AND** clicking the button updates the displayed count
- **AND** closing the window exits cleanly

#### Scenario: Widget hit-testing is testable

- **WHEN** package tests construct a button with a known bounds rectangle
- **THEN** hit-testing reports click inside/outside deterministically without
  requiring a visible window where tests are structured as pure logic

#### Scenario: Button click can be tested without manual UI

- **WHEN** package tests construct a mouse-button event whose coordinates fall
  inside a button
- **THEN** `Button.handle_event` reports activation deterministically
- **AND** the counter demo's state update can be covered without relying only
  on a human clicking a visible window

### Requirement: sggui SHALL share event and frame semantics with sggame

The `sggui` package SHALL use the same `sgplatform` event pump and present path
as `sggame` so applications do not mix conflicting main-loop implementations.

#### Scenario: GUI loop uses platform events

- **WHEN** an `sggui` application runs its main loop
- **THEN** quit and mouse button events are obtained through `sgplatform`
- **AND** no duplicate SDL initialization occurs in `sggui` beyond `sgplatform`

### Requirement: sggui SHALL document GUI limitations explicitly

The `sggui` README and support matrix SHALL state that phase 1 is self-drawn,
not native-themed controls, and SHALL list deferred features such as text input
fields, menus, and native file dialogs.

#### Scenario: README sets expectations

- **WHEN** a reviewer reads `sggui` documentation
- **THEN** they can distinguish supported widgets from deferred ones
- **AND** they can find the counter example as the canonical GUI demo
