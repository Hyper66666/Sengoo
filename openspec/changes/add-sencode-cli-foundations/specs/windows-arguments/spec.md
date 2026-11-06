## ADDED Requirements

### Requirement: Strict Unicode OS arguments
The Windows standard argument boundary SHALL parse the OS UTF-16 command line
and provide owned strict UTF-8 values, preserving quoted, empty and trailing
backslash arguments while excluding the executable from user argument indexes.
The implementation SHALL clear owned values on reinitialization and exit and
report initialization failures without falling back to lossy ANSI arguments.

#### Scenario: Unicode arguments independent of active codepage
- **WHEN** a compiled Sengoo program receives Chinese, supplementary Unicode,
  quotes, spaces, a trailing backslash and an empty argument
- **THEN** `args_len`, `arg_len` and `arg_copy` preserve every original value
- **AND** managed `arg_copy` records the copied initialized length, including zero,
  so `string_from_buffer` accepts the initialized UTF-8 bytes
- **AND** out-of-range indexes return stable errors without integer overflow

#### Scenario: Unicode caller working directory
- **WHEN** a program starts in a Windows working directory containing Unicode
- **THEN** current-directory length and copy functions return strict UTF-8
- **AND** the decoded value preserves the OS directory name without ANSI loss
- **AND** directory existence and creation use strict UTF-16 OS paths

#### Scenario: Managed environment value copying
- **WHEN** a Windows environment name or value contains Unicode or is empty
- **THEN** the environment API reads the OS wide value and returns strict UTF-8
- **AND** the managed copy records its initialized byte length for safe String construction

#### Scenario: Windows argument initialization fails
- **WHEN** OS argument parsing, strict UTF-8 conversion or allocation fails
- **THEN** the argument functions return a negative existing status
- **AND** all partially created native allocations are released
