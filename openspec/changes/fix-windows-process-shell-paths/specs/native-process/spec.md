## ADDED Requirements

### Requirement: Windows shell program paths and command tails preserve intended execution
Windows ProcessCommand SHALL accept slash-separated executable paths and retain
the command processor grammar for an explicitly selected cmd /c or /k final
command payload, without rewriting other program arguments or POSIX paths.

#### Scenario: Forward-slash system command processor
- **WHEN** a caller explicitly spawns system cmd.exe using forward slashes
- **THEN** a simple echo command completes with exit0 and captured output
- **AND** the child and process builder resources are released normally

#### Scenario: Quoted shell command and URL
- **WHEN** the final cmd /c payload contains a quoted value and a slash-separated URL
- **THEN** shell parsing preserves the intended quoted value and URL
- **AND** other executable argv retains its existing CRT encoding

#### Scenario: Unicode working directory and cancellation
- **WHEN** a process runs from a Unicode path or requires timeout cancellation
- **THEN** the existing Unicode, bounded output and process-tree cleanup contracts still hold
