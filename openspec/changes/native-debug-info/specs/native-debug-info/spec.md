## ADDED Requirements

### Requirement: `sgc` SHALL emit standard debug metadata under the debug flag

`sgc build` and `sgc run` SHALL accept `-g` (`--debug-info`) and, when set,
emit LLVM debug-info metadata — compile units, per-function subprograms, and
statement locations — in the textual IR so the pinned `clang` toolchain
produces DWARF on POSIX targets and CodeView on Windows targets.

#### Scenario: Debug metadata is present under `-g`

- **WHEN** a program is built with `sgc build -g`
- **THEN** the emitted IR contains a `DICompileUnit`, a `DIFile` for each
  Sengoo source, and a `DISubprogram` attached to every user function
  including synthesized lambda functions
- **AND** calls, branches, returns, and assignments carry `!dbg` locations
  whose file and line match the source spans
- **AND** the required debug-info module flags for the pinned toolchain are
  present

#### Scenario: The default pipeline is unchanged without `-g`

- **WHEN** the same program is built without `-g`
- **THEN** the emitted IR is byte-identical to the pre-change baseline
- **AND** no debug metadata or module flags are added

### Requirement: Source-line breakpoints SHALL bind and step in native debuggers

A binary built with `-g` SHALL support setting a breakpoint on a Sengoo
source file and line, hitting it, and stepping by source line in the
documented native debugger on each supported host.

#### Scenario: Breakpoint binds and hits on Linux

- **WHEN** a developer loads a `-g` artifact in lldb and sets a breakpoint
  by Sengoo file and line
- **THEN** the breakpoint resolves to an address and the program stops at
  that source location when run
- **AND** `next` advances to the following Sengoo source line

#### Scenario: Breakpoint binds and hits on Windows

- **WHEN** a developer loads a `-g` artifact in WinDbg or cdb and sets a
  source-line breakpoint
- **THEN** the CodeView path resolves and hits the breakpoint at the same
  source location
- **AND** source-line stepping advances through Sengoo lines

#### Scenario: Stepping does not jump to unlocated code

- **WHEN** a developer steps through a function whose lowered instructions
  had no direct span
- **THEN** those instructions inherit the enclosing statement location
  rather than reporting line 0 or leaving source view

### Requirement: Debug builds SHALL preserve semantics and cache separation

Enabling `-g` SHALL NOT change program results, and debug artifacts SHALL
never be served from or stored into the non-debug artifact cache entries.

#### Scenario: Conformance results are unchanged under `-g`

- **WHEN** the pinned conformance forms are built and run with `-g` through
  the real-CLI gate
- **THEN** every form produces the same exit code and stdout as the
  non-debug build

#### Scenario: Debug and non-debug artifacts never alias

- **WHEN** the same source is built with and without `-g`
- **THEN** the artifact cache stores them under distinct fingerprints
- **AND** a `-g` request never reuses a non-debug artifact or vice versa

### Requirement: Debugger documentation SHALL cover the source-level workflow

`docs/debugging-native.md` SHALL document building with `-g` and the
breakpoint/stepping workflow per host, backed by committed transcripts.

#### Scenario: A developer follows the debugging guide

- **WHEN** a developer follows `docs/debugging-native.md` on a supported
  host
- **THEN** the guide shows how to build with `-g`, attach or launch the
  documented debugger, set a source-line breakpoint, and step
- **AND** the guide links validated lldb and WinDbg/cdb transcripts
- **AND** the support matrix row for source-level debugging cites them
