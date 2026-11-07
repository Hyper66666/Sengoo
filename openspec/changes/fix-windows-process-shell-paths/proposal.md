# Windows process shell paths

## Why

The real Sencode CLI executor fixture demonstrated that cmd.exe rejects a
forward-slash executable path even for echo. A separate Windows subprocess
reproduction confirmed the same failure outside Sengoo. Its shell command tail
also cannot use CRT backslash escaping for embedded quotes.

## Capabilities

Normalize Windows ProcessCommand executable separators before launch and retain
cmd /c or /k single command-tail quoting through the shared process builder.
All command arguments for other executables remain unchanged. Keep ABI1,
edition2026, published interfaces and POSIX behavior compatible.

## Non-Goals

Implicit shell execution, changing user scripts/arguments, PATH discovery,
credential changes or a compiler syntax change.

## Ownership

One integration owner changes the general native process boundary. CLI agent
logic, diagnostics, retry scheduling and batch edits remain in Sengoo source
in the adjacent Sencode-CLI repository.
