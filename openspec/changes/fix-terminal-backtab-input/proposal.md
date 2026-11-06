# Normalize standard BackTab input

## Capabilities

The shared terminal byte decoder recognizes the standard BackTab sequence
`ESC [ Z` as key code 9 with Shift modifier bit 1, matching native Windows
Shift+Tab events. Partial input keeps the existing escape timeout and byte
bounds. The change is a reusable terminal boundary needed by Sencode's
permission-mode shortcut; application permission policy remains Sengoo code.

## Non-Goals

No extended keyboard protocol, new terminal ABI, application shortcut, terminal
renderer or new platform qualification is introduced. Only the documented
standard sequence is added; unsupported parameterized variants are unchanged.

## Ownership

This change alone owns BackTab normalization in `runtime_terminal.c`. Native
decoder and hidden-console probes plus a public Sengoo redirected-input consumer
verify the common event contract. The CLI integration owns its shortcut UI.
