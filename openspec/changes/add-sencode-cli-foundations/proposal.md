# Sencode-driven native CLI foundations

## Why
The native Sencode CLI is an external consumer of Sengoo. Its streaming model
requests, interactive terminal and cancellable tools expose missing generic I/O
capabilities. Existing v0.2 programs and runtime ABI 1 remain compatible.

## Capabilities
- Correct JSON Unicode, bounded cancellable TLS byte transport.
- Piped process lifecycle and terminal events with owned cleanup.
- Interprocess locks, Windows credential-store access, package improvements.
- Compiler regressions reduced from real CLI programs.

## Non-Goals
No provider, agent, goal or MCP business logic in native runtime code. No TLS or
SQLite reimplementation, lifetime-system redesign or required cross-platform CLI
release. Native application logic belongs to the Sencode repository.

## Baseline and ownership
Sengoo: d46b3f6cd8d6e770734f98edc57aa67cb931e26a.
Sencode: a468086401a0d19077b5cf0b6aea3ecff0f04c9a.
Network/data owner: network agent; process/terminal owner: process agent;
package/compiler integration and shared OS primitives: root integrator.
