## ADDED Requirements

### Requirement: Fullscreen terminal ownership SHALL unwind without changing legacy events
Terminal SHALL expose enter_fullscreen and leave_fullscreen returning
Result<bool, i64>. Enter and leave SHALL be idempotent. Fullscreen entry SHALL
save whether raw and mouse modes were already owned. Leaving SHALL restore
those prior modes. Explicit restore, Drop and normal process exit SHALL restore
the original screen, input modes and Windows codepages. Redirected handles SHALL
report unsupported. The existing TerminalEvent layout and kinds 0 through 5
SHALL remain compatible.

#### Scenario: Fullscreen owns raw mode
- **WHEN** a TTY enters fullscreen without a preceding raw-mode operation
- **THEN** leaving fullscreen restores the original console modes
- **AND** a second leave succeeds without altering the console

#### Scenario: Fullscreen borrows existing raw mode
- **WHEN** raw mode was entered before fullscreen
- **THEN** leaving fullscreen retains raw mode until restore or Drop

### Requirement: Mouse input SHALL expose bounded generic events
Terminal SHALL expose explicit mouse tracking and a separate mouse snapshot.
Events SHALL report zero-based viewport coordinates, buttons, modifiers,
press/release/move/wheel and independent scroll axes. Native input parsing SHALL
remain bounded and report malformed or overflowing encodings explicitly.

#### Scenario: A native mouse event arrives between Unicode keys
- **WHEN** a press, drag, wheel or release is queued between Unicode keys
- **THEN** each event retains its own coordinates and modifiers
- **AND** subsequent Unicode key and paste decoding remains intact
