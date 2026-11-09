## ADDED Requirements

### Requirement: Synchronous Windows ProcessCommand launches SHALL preserve UTF-8

Windows `ProcessCommand.run` and its pipeline launches SHALL preserve valid
UTF-8 executable paths, arguments, working directories and environment edits
through wide Windows process APIs without depending on the active ANSI code
page. Existing CRT quoting, explicit shell behavior, capture and timeout
semantics SHALL remain compatible.

#### Scenario: Unicode executable and working directory

- **WHEN** a synchronous process is launched from a Unicode executable path with a Unicode working directory, arguments and environment edits
- **THEN** the actual child observes the intended values
- **AND** spaces, quotes, literal ampersands, empty arguments and trailing backslashes retain their argument boundaries

#### Scenario: Pipeline and timeout still work

- **WHEN** a synchronous pipeline transfers Unicode bytes or a child exceeds its original configured timeout
- **THEN** pipeline input/output bytes and timeout status retain their existing meaning
- **AND** owned native and OS resources return to baseline after close

### Requirement: Invalid UTF-8 launch input SHALL fail before child creation

Malformed UTF-8 in a Windows synchronous command line, working directory or
environment edit SHALL fail through the existing failed-launch result before
creating a child. The failure SHALL NOT substitute lossy text or change runtime
ABI1. Temporary conversion allocations SHALL be released on failure.

#### Scenario: Invalid executable, argument, directory or environment text

- **WHEN** a synchronous launch receives malformed UTF-8 in any of those input positions
- **THEN** launch fails without a child output handle
- **AND** repeated rejected launches preserve exact tracked and OS handle baselines

### Requirement: Native launch evidence SHALL identify the runtime actually exercised

Regression evidence SHALL bind source fingerprints, host, seed/native tools,
commands, exit status and output to real native and SG process tests. It SHALL
distinguish this source-runtime correction from a new seed or self-hosted
compiler claim.

#### Scenario: Source-runtime launch correction is verified

- **WHEN** the native probe and SG process test run with the recorded tools and source runtime
- **THEN** Unicode children verify their actual argv, directory and environment
- **AND** success includes repeated exact resource recovery and matching source fingerprints
