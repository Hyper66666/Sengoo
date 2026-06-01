# stdlib-mainstream-usability Specification

## Purpose
Defines portable standard-library APIs and toolchain wiring requirements for common scripting, CLI, filesystem, and process workflows.
## Requirements
### Requirement: Standard library modules SHALL be wired through compiler, CLI, LSP, docs, and examples
Every new source-level standard-library module SHALL be available through `sgc` stdlib import expansion, `sglsp` stdlib symbol/signature discovery, stdlib docs, and a runnable example.

#### Scenario: A new stdlib module is imported by a program
- **WHEN** a Sengoo program imports `std::<module>`
- **THEN** `sgc check`, `sgc build`, and `sgc run` preload the module and its declared source dependencies
- **AND** `sglsp` exposes the module's public symbols and signatures when the import is present
- **AND** `examples/stdlib` contains a runnable example for the module

### Requirement: Path utilities SHALL support common cross-platform path operations
The standard library SHALL provide `std::path` helpers for path separator discovery, absolute-path checks, joining, file-name/stem/extension extraction, parent extraction, and lexical normalization.

#### Scenario: A program manipulates paths without raw pointer choreography
- **WHEN** a Sengoo program imports `std::path`
- **THEN** it can call safe wrappers using `&str` inputs and managed `Buffer` outputs
- **AND** fallible string-producing helpers return `Result<i64, i64>` with the byte count on success

#### Scenario: Absolute paths are recognized conservatively
- **WHEN** a program checks a Unix root path, a Windows drive-root path, or a UNC-like path
- **THEN** `path_is_absolute` returns true
- **AND** relative paths return false

#### Scenario: Path normalization is lexical
- **WHEN** a program normalizes a path containing duplicate separators, `.` segments, or simple `..` segments
- **THEN** the result is normalized lexically into the provided `Buffer`
- **AND** the helper does not resolve symlinks or require the path to exist on disk

#### Scenario: Joining with an absolute right-hand side
- **WHEN** a program joins a base path with a right-hand side that is already absolute
- **THEN** `path_join` writes the absolute right-hand side into the provided `Buffer`
- **AND** it does not prefix the base path

### Requirement: Runtime-produced string outputs SHALL use managed Buffer handles
Until Sengoo has a specified owned-string return ABI, stdlib runtime helpers that produce string-like output SHALL copy into managed `Buffer` handles and report byte counts.

#### Scenario: A helper produces a string-like result
- **WHEN** a stdlib helper such as `path_join`, `path_parent`, or `path_extension` needs to return text
- **THEN** it accepts a managed `Buffer`
- **AND** it returns `Result<i64, i64>` indicating bytes written or an error code

### Requirement: Process utilities SHALL expose portable process metadata without command execution
The standard library SHALL provide `std::process` helpers for process ID, current working directory length/copy, and conventional exit-code selection.

#### Scenario: A program inspects the current process
- **WHEN** a Sengoo program imports `std::process`
- **THEN** it can call `process_id()` to get a positive process identifier
- **AND** it can call `process_current_dir_len()` and `process_current_dir_copy(buffer)` to copy the current working directory into a managed `Buffer`
- **AND** fallible string-producing helpers return `Result<i64, i64>` with the byte count on success

#### Scenario: A program maps a boolean success value to an exit code
- **WHEN** a program calls `process_exit_code(true, failure_code)`
- **THEN** it returns `0`
- **AND** `process_exit_code(false, failure_code)` returns `failure_code`

### Requirement: Process and data-format usability SHALL be gated by explicit follow-up design
Process execution and JSON-like data-format helpers SHALL NOT be added opportunistically in the path phase.

#### Scenario: A later phase proposes command execution or JSON helpers
- **WHEN** a future implementation needs process execution or JSON-like parsing/formatting
- **THEN** it first updates OpenSpec with API shape, portability constraints, security constraints, and tests

#### Scenario: A later phase proposes additional process or entry ABI features
- **WHEN** a future implementation needs process execution, environment mutation, or a command-line surface beyond `std::args`
- **THEN** it first updates OpenSpec with API shape, portability constraints, security constraints, ABI changes, and tests

### Requirement: Command-line arguments SHALL be available through an opt-in stdlib module
The standard library SHALL provide `std::args` helpers for counting user-supplied command-line arguments and copying individual argument text into managed `Buffer` handles.

#### Scenario: The args API exposes user arguments only
- **WHEN** a program imports `std::args`
- **THEN** it can call `args_len()`, `arg_exists(index)`, `arg_len(index)`, and `arg_copy(index, buffer)`
- **AND** index `0` refers to the first user-supplied argument after the executable or source path

#### Scenario: A program reads arguments passed through `sgc run`
- **WHEN** a user runs `sgc run program.sg -- alpha beta`
- **THEN** a program importing `std::args` observes `args_len() == 2`
- **AND** `arg_len(0)` returns the byte length of `alpha`
- **AND** `arg_copy(0, buffer)` copies `alpha`
- **AND** `arg_copy(1, buffer)` copies `beta`

#### Scenario: Runtime args do not affect compile artifact reuse
- **WHEN** a user runs the same source through `sgc run` with different trailing arguments
- **THEN** source hashing, object reuse, and relinking decisions remain based on source and compiler inputs rather than argument values
- **AND** each invocation still observes the current trailing arguments at runtime

#### Scenario: A native binary reads direct command-line arguments
- **WHEN** a user builds a native binary from a program importing `std::args`
- **AND** runs the binary as `program alpha beta`
- **THEN** `args_len() == 2`
- **AND** argument index `0` is `alpha`, not the executable path

#### Scenario: A program does not use the args runtime
- **WHEN** a Sengoo program does not call `std::args` helpers
- **THEN** compiler output preserves the existing zero-argument `main` function shape

#### Scenario: An argument index is out of range
- **WHEN** a program calls `arg_len(index)` or `arg_copy(index, buffer)` with a negative or out-of-range index
- **THEN** the helper returns an error-shaped `Result`

### Requirement: Collection ergonomics SHALL document currently supported runtime-backed shapes
The standard library examples SHALL include a first-class `std::collections` example for the currently supported runtime-backed `Vec<T>`, `HashMap<K, V>`, and iterator helpers.

#### Scenario: A user looks for collection examples
- **WHEN** a user opens `examples/stdlib`
- **THEN** the catalog includes a runnable `std::collections` example
- **AND** the example uses currently supported runtime-backed scalar shapes rather than implying unsupported string containers

#### Scenario: A later phase proposes generic data-format helpers or string collections
- **WHEN** a future implementation needs JSON-like parsing/formatting or `Vec<&str>` / `HashMap<&str, ...>` support
- **THEN** it first updates OpenSpec with the required value, string, byte-slice, and ownership model

### Requirement: Standard I/O utilities SHALL support synchronous pipeline-style programs
The standard library SHALL provide `std::io` helpers for bounded stdin reads,
stdout writes, stderr writes, and stream flushing.

#### Scenario: A program reads stdin into a managed Buffer
- **WHEN** a Sengoo program imports `std::io`
- **THEN** it can call `io_stdin_read(buffer)` to read up to the buffer capacity
- **AND** it can call `io_stdin_read_line(buffer)` to read up to the buffer
  capacity or through one newline
- **AND** successful reads return `Result<i64, i64>` with the byte count
- **AND** EOF without bytes is a successful read count of `0`

#### Scenario: A program writes exact bytes to stdout and stderr
- **WHEN** a Sengoo program imports `std::io`
- **THEN** it can call `io_stdout_write(data)` and `io_stderr_write(data)`
- **AND** the helpers write exactly the provided string bytes without adding a newline
- **AND** successful writes return `Result<i64, i64>` with the byte count

#### Scenario: A program flushes standard output streams
- **WHEN** a Sengoo program imports `std::io`
- **THEN** it can call `io_stdout_flush()` and `io_stderr_flush()`
- **AND** successful flushes return `Result<bool, i64>` with `true`

#### Scenario: Standard I/O helpers are wired through the stdlib toolchain
- **WHEN** a Sengoo program imports `std::io`
- **THEN** `sgc check`, `sgc build`, and `sgc run` preload the module and its
  declared source dependencies
- **AND** `sglsp` exposes the module's public symbols and signatures
- **AND** `examples/stdlib` contains a runnable synchronous I/O example

#### Scenario: Advanced I/O features remain explicitly deferred
- **WHEN** a future implementation needs async I/O, terminal control, file
  descriptor APIs, or owned-string stdin helpers
- **THEN** it first updates OpenSpec with API shape, portability constraints,
  ownership rules, and tests

### Requirement: String conversion helpers SHALL parse and format decimal i64 values
The standard library SHALL provide a source-level `std::strconv` module for
portable decimal `i64` parsing and formatting.

#### Scenario: A program parses a decimal i64 string
- **WHEN** a Sengoo program imports `std::strconv`
- **AND** calls `strconv_parse_i64("  -42\n")`
- **THEN** the helper returns an ok-shaped `Result<i64, i64>` with value `-42`

#### Scenario: A program parses bytes read into a managed Buffer
- **WHEN** a program has a managed `Buffer` containing decimal ASCII bytes
- **AND** calls `strconv_parse_i64_buffer(buffer, len)` with the number of
  meaningful bytes
- **THEN** the helper parses only that byte range
- **AND** returns an ok-shaped `Result<i64, i64>` with the parsed value

#### Scenario: Invalid or overflowing input is rejected
- **WHEN** a program parses empty input, non-numeric input, input with
  non-whitespace trailing characters, or an overflowing decimal integer
- **THEN** the helper returns an error-shaped `Result<i64, i64>`

#### Scenario: A program formats an i64 into a managed Buffer
- **WHEN** a program calls `strconv_format_i64(value, buffer)`
- **THEN** the helper writes the base-10 ASCII representation into the Buffer
- **AND** returns an ok-shaped `Result<i64, i64>` with the number of bytes
  written
- **AND** it does not append a NUL terminator

#### Scenario: Advanced conversion features remain explicitly deferred
- **WHEN** a future implementation needs floats, radix-specific parsing,
  locale-specific formatting, arbitrary precision integers, owned-string
  returns, or JSON/data-format conversion
- **THEN** it first updates OpenSpec with API shape, ownership constraints,
  portability constraints, and tests

### Requirement: Directory utilities SHALL support safe portable setup operations
The standard library SHALL provide `std::dir` helpers for directory existence,
single-directory creation, recursive directory creation, and empty-directory
removal.

#### Scenario: A program prepares an output directory
- **WHEN** a Sengoo program imports `std::dir`
- **THEN** it can call `dir_exists(path)` to test for a directory
- **AND** it can call `dir_create(path)` to create one directory
- **AND** it can call `dir_create_all(path)` to create missing parent directories
- **AND** successful fallible helpers return `Result<bool, i64>` with `true`

#### Scenario: Directory creation is idempotent
- **WHEN** a program calls `dir_create(path)` or `dir_create_all(path)` for a
  directory that already exists
- **THEN** the helper returns a successful `Result<bool, i64>`

#### Scenario: Empty directory removal is bounded
- **WHEN** a program calls `dir_remove(path)` for an empty directory
- **THEN** the helper removes that directory and returns a successful
  `Result<bool, i64>`
- **AND** the helper does not recursively delete populated directory trees

#### Scenario: Directory helpers are wired through the stdlib toolchain
- **WHEN** a Sengoo program imports `std::dir`
- **THEN** `sgc check`, `sgc build`, and `sgc run` preload the module and its
  declared source dependencies
- **AND** `sglsp` exposes the module's public symbols and signatures
- **AND** `examples/stdlib` contains a runnable directory example

### Requirement: Directory utilities SHALL support deterministic non-recursive listing
The standard library SHALL provide `std::dir` helpers for counting immediate
directory entries and copying one entry name into a managed Buffer.

#### Scenario: A program counts immediate directory entries
- **WHEN** a Sengoo program imports `std::dir`
- **AND** calls `dir_entry_count(path)` on a readable directory
- **THEN** the helper returns an ok-shaped `Result<i64, i64>` containing the
  number of immediate child entries
- **AND** the count excludes `.` and `..`

#### Scenario: A program copies a deterministic entry name
- **WHEN** a directory contains entries named `b.txt` and `a.txt`
- **AND** a program calls `dir_entry_name(path, 0, buffer)`
- **THEN** the helper copies `a.txt` into the managed Buffer
- **AND** returns an ok-shaped `Result<i64, i64>` with the number of bytes
  copied
- **AND** it does not append a NUL terminator

#### Scenario: Listing order is stable across host iteration order
- **WHEN** a directory contains multiple entries
- **THEN** `dir_entry_name` indexes entries after sorting names by unsigned
  byte order

#### Scenario: Invalid listing requests are rejected
- **WHEN** a program lists a non-directory path, uses a negative or out-of-range
  index, or provides an output Buffer that is too small
- **THEN** the helper returns an error-shaped `Result<i64, i64>`

#### Scenario: Advanced directory traversal features remain explicitly deferred
- **WHEN** a future implementation needs recursive traversal, recursive
  deletion, glob matching, metadata structs, owned-string entry returns, or a
  persistent iterator/list API
- **THEN** it first updates OpenSpec with API shape, ownership constraints,
  portability constraints, safety constraints, and tests

### Requirement: File utilities SHALL support explicit-overwrite copy and move
The standard library SHALL provide `std::file` helpers for copying file bytes
and moving files with an explicit overwrite choice.

#### Scenario: A program copies a file without removing the source
- **WHEN** a Sengoo program imports `std::file`
- **AND** calls `file_copy(source, destination, false)` for a readable source
  and absent destination
- **THEN** the helper writes the same bytes to the destination
- **AND** returns an ok-shaped `Result<i64, i64>` with the number of bytes
  copied
- **AND** the source remains present

#### Scenario: Existing destinations require explicit overwrite
- **WHEN** a destination already exists
- **AND** a program calls `file_copy(source, destination, false)` or
  `file_move(source, destination, false)`
- **THEN** the helper returns an error-shaped result
- **AND** does not replace the destination

#### Scenario: A program explicitly overwrites a copied destination
- **WHEN** a destination already exists
- **AND** a program calls `file_copy(source, destination, true)`
- **THEN** the destination bytes are replaced with the source bytes
- **AND** the helper returns the number of bytes copied

#### Scenario: A program cannot copy a file onto itself
- **WHEN** source and destination refer to the same host file
- **AND** a program calls `file_copy(source, destination, overwrite)`
- **THEN** the helper returns an error-shaped result
- **AND** leaves the source bytes intact

#### Scenario: A program moves a file with host rename semantics
- **WHEN** a program calls `file_move(source, destination, overwrite)`
- **AND** the host rename primitive succeeds
- **THEN** the helper returns an ok-shaped `Result<bool, i64>` containing
  `true`
- **AND** the source path no longer exists
- **AND** the destination path exists

#### Scenario: Advanced file-transfer features remain explicitly deferred
- **WHEN** a future implementation needs recursive directory transfer,
  cross-filesystem move fallback, metadata preservation guarantees, atomic
  copy guarantees, progress callbacks, cancellation, or async I/O
- **THEN** it first updates OpenSpec with API shape, portability constraints,
  safety constraints, and tests

### Requirement: Process utilities SHALL support synchronous shell-free child execution
The standard library SHALL provide `std::process` helpers for running a child
executable directly with zero through three explicit string arguments.

#### Scenario: A program runs a child executable and reads its exit code
- **WHEN** a Sengoo program calls `process_run(executable)` or a fixed-arity
  `process_run_1` through `process_run_3` helper
- **AND** the host starts and waits for the executable successfully
- **AND** the child exits normally
- **THEN** the helper returns an ok-shaped `Result<i64, i64>` containing the
  child exit code
- **AND** a nonzero child exit code remains a successful process-run result

#### Scenario: Arguments remain literal child argv entries
- **WHEN** a program passes an argument containing spaces or shell
  metacharacters to a fixed-arity process helper
- **THEN** the runtime passes that value as one literal child argument
- **AND** the runtime does not interpret the value as shell syntax

#### Scenario: Process execution inherits standard streams
- **WHEN** a program runs a child executable
- **THEN** the child inherits the current process stdin, stdout, and stderr
- **AND** the helper blocks until the child exits

#### Scenario: Invalid or failed execution returns an error-shaped result
- **WHEN** the executable path is empty, a used raw argument pointer is
  missing, the argument count is outside zero through three, startup fails,
  waiting fails, or the child does not exit normally
- **THEN** the helper returns an error-shaped result

#### Scenario: Advanced process management remains explicitly deferred
- **WHEN** a future implementation needs arbitrary-length argv, implicit shell
  commands, stream capture, pipes, cwd or environment overrides, background
  handles, timeouts, signals, cancellation, or async execution
- **THEN** it first updates OpenSpec with API shape, portability constraints,
  security constraints, lifecycle semantics, and tests
