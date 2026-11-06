# Local terminal screens, byte IPC and reusable text/artifact foundations

## Why
The approved Sencode local CLI plan needs an independently owned fullscreen
terminal frontend and an execution host. These expose generic terminal and
local byte-transport gaps in Sengoo rather than application-specific runtime
requirements.

## Scope and ownership
The terminal/IPC implementation agent owns the terminal-screen-events and
local-byte-ipc deltas in this change. The existing add-sencode-cli-foundations
change retains ownership of piped processes and the original terminal events.
The root integrator owns the Sencode application protocol and integration.
The root integrator also owns the additive OS current-executable path API and
its independent `current_executable` regression. This change owns that single
requirement; the original system foundation keeps its preceding requirements.

Add fullscreen and mouse operations without changing the six-field
TerminalEvent layout or runtime ABI 1. Add Windows named-pipe owning byte APIs
limited to the current user and logon session. Framing, version handshakes,
request identities, authentication policy and agent state remain Sengoo code.

The same implementation owner adds generic Windows clipboard output, Unicode
17 extended-grapheme boundaries and terminal-width policy in pure Sengoo, and
bounded streaming SHA-256 file hashing. The shared SHA compression core is reused;
these native additions contain no provider, agent or packaged-runtime policy.

This owner also closes a reproduced lifetime gap in the three owned String
snapshot iterators: scope exit SHALL free their allocations, explicit free
remains available, and opaque generation checks reject stale native tokens.
The existing piped-process requirement remains owned by
add-sencode-cli-foundations; this implementation adds its explicit
try_write_stdin alias and an independent stopped-reader pressure regression.

## Boundaries
Windows x64 is the first verification host. Other platform implementations or
unsupported results are documented independently; no unrun platform is marked
verified. Forced process termination is outside normal terminal cleanup.
