# Owning Windows pseudoconsole processes

## Why
The approved Sencode P3 terminal-pane work requires interactive child consoles
without borrowing the user's current terminal. This change adds a generic
Windows ConPTY boundary to Sengoo; terminal emulation, scrollback, agent policy
and pane layout remain Sengoo application code.

## Scope and owner
The platform implementation agent owns this change and its `std::pty` module,
native runtime source, registrations and independent probes. It does not modify
Sencode application files or replace the existing piped-process foundation.
The root integrator owns the application pane adapter and end-to-end evidence.

## Compatibility
Preserve edition 2026 and runtime ABI 1. All interfaces are additive. Windows
ConPTY availability is detected at runtime; other platforms and older Windows
return unsupported. Only Windows x64 execution is qualified in this change.
