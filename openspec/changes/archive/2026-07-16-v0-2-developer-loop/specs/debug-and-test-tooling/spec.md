## ADDED Requirements

### Requirement: The editor SHALL launch the retained native debug path

The VS Code workflow SHALL discover the installed Sengoo artifact and launch it
through the documented CDB/WinDbg or LLDB adapter/configuration while preserving
Sengoo source paths emitted by `native-debug-info`.

#### Scenario: Developer debugs a package test or binary

- **WHEN** a supported host starts a Sengoo debug session from the editor
- **THEN** the selected package target is built with debug info
- **AND** source breakpoints bind, stepping follows Sengoo lines, and the
  supported stack/local subset is visible

#### Scenario: Required adapter is unavailable

- **WHEN** the documented native debugger adapter is not installed
- **THEN** the editor reports the missing adapter and setup documentation
- **AND** does not pretend the session started successfully

### Requirement: Structured test failures SHALL navigate to source

The editor SHALL consume the versioned `sgc test` result/assertion envelope and
associate each failure with its exact source location without parsing
human-readable stderr text.

#### Scenario: Assertion fails in an open package

- **WHEN** `sgc test` reports a structured assertion with file and line/range
- **THEN** the editor publishes a diagnostic and navigation target at that
  source location
- **AND** existing result fields remain backward compatible

### Requirement: One installed package SHALL prove the complete developer loop

An installed release SHALL exercise one package through editing, semantic
navigation, rename, formatting, checking, testing, debugging, and documentation
using the same tool versions.

#### Scenario: M2 is proposed for archive

- **WHEN** `v0-2-developer-loop` reaches its archive gate
- **THEN** one E2E records exact tool versions/paths and completes every operation
- **AND** no operation depends on repository-relative binaries or untracked data
