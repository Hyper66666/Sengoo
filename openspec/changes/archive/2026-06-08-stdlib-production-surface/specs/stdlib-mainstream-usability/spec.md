## MODIFIED Requirements

### Requirement: Runtime-produced string outputs SHALL use managed Buffer handles

Stdlib helpers that already accept caller-supplied `Buffer` outputs SHALL keep
their current names and `Result<i64, i64>` byte-count contracts. This change adds
parallel owned-`String` helpers rather than replacing Buffer workflows.

#### Scenario: Legacy Buffer helpers remain source-compatible

- **WHEN** a program calls existing helpers such as `path_join`, `path_parent`, or
  `path_extension` with a managed `Buffer`
- **THEN** behavior and signatures remain unchanged from the canonical baseline
- **AND** examples that use Buffer workflows continue to compile

## ADDED Requirements

### Requirement: Stdlib SHALL expose additive owned-text production helpers

Sengoo SHALL add the following additive helpers without renaming existing
Buffer-based APIs:

| Helper | Result |
| --- | --- |
| `path_join_string`, `path_normalize_string`, `path_parent_string`, `path_file_name_string`, `path_stem_string`, `path_extension_string`, `dir_entry_name_string` | `Result<String, i64>` |
| `JsonValue.string_value()`, `json_value_as_string(value)` | `Result<String, i64>` |
| `vec_new_string()`, `Vec<String>`, `StringMapString` | owned collection semantics below |
| `dir_walk`, `dir_copy_tree`, `dir_remove_tree` | bounded recursive IO |
| `ProcessCommand.pipe_stdout_to(child)`, `ProcessCommand.spawn()`, `ProcessHandle` | shell-free process semantics below |
| `io_fd_read(fd, buffer)`, `io_fd_write(fd, data)` | sync fd subset |

Existing `json_parse(text: &str)` and `json_parse_buffer(buffer, input_len)` remain
the canonical JSON input APIs. This change does not add a redundant
`json_parse_string` alias.

#### Scenario: Owned path helpers return String without a caller Buffer

- **WHEN** a program calls `path_join_string` or `dir_entry_name_string`
- **THEN** success returns `Result<String, i64>`
- **AND** invalid UTF-8 or host failures map to `STATUS_INVALID_ARGUMENT` or `STATUS_IO`

#### Scenario: String collections use move-in and clone-on-read semantics

- **WHEN** a program uses `Vec<String>` or `StringMapString`
- **THEN** `push`/`insert` move owned values in, `get` returns clones, and `remove`
  transfers the stored value out
- **AND** invalid handles return `STATUS_INVALID_HANDLE`

#### Scenario: JSON input cap increases to at least 1 MiB

- **WHEN** a program parses JSON up to the new default cap of at least 1 MiB
- **THEN** valid documents parse successfully
- **AND** oversize input returns a stable oversize status without crashing

#### Scenario: Recursive directory helpers are bounded and do not follow symlinks by default

- **WHEN** a program calls `dir_walk`, `dir_copy_tree`, or `dir_remove_tree`
- **THEN** default limits are max depth 64 and max entries 100000 unless the caller
  supplies stricter limits
- **AND** symlinks are not followed by default

#### Scenario: Process pipes and background handles remain shell-free

- **WHEN** a program uses `ProcessCommand.pipe_stdout_to(child)`
- **THEN** success consumes both command values and returns the final command owning
  the pipeline chain
- **AND** `run()` reports the final stage `ProcessOutput`
- **WHEN** a program uses `ProcessCommand.spawn()`
- **THEN** `ProcessHandle.wait(timeout_ms)` returns the exit code or `STATUS_TIMEOUT`,
  `kill()` returns `Result<bool, i64>`, `exit_code()` is valid only after completion,
  and `close()` releases the handle
- **AND** behavior is verified on Windows and POSIX CI hosts
