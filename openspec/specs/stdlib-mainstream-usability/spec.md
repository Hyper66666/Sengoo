# stdlib-mainstream-usability Specification

## Purpose
Defines portable standard-library APIs and toolchain wiring requirements for common scripting, CLI, filesystem, and process workflows.
## Requirements
### Requirement: Standard library modules SHALL be wired through compiler, CLI, LSP, docs, and examples

Every new or stabilized source-level standard-library module SHALL be available
through `sgc` stdlib import expansion, `sglsp` stdlib symbol/signature
discovery, stdlib docs, and a runnable example. Stabilizing an existing partial
module such as `std::net` SHALL include compatibility notes for old names that
remain supported but are not the preferred public surface.

#### Scenario: A new stdlib module is imported by a program

- **WHEN** a Sengoo program imports `std::<module>`
- **THEN** `sgc check`, `sgc build`, and `sgc run` preload the module and its declared source dependencies
- **AND** `sglsp` exposes the module's public symbols and signatures when the import is present
- **AND** `examples/stdlib` contains a runnable example for the module

#### Scenario: An existing partial module is stabilized

- **WHEN** this change stabilizes an existing partial module such as `std::net`
- **THEN** docs identify stable public names and compatibility-only names
- **AND** examples use stable public names

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

Stdlib helpers that already accept caller-supplied `Buffer` outputs SHALL keep
their current names and `Result<i64, i64>` byte-count contracts. This change adds
parallel owned-`String` helpers rather than replacing Buffer workflows.

#### Scenario: Legacy Buffer helpers remain source-compatible

- **WHEN** a program calls existing helpers such as `path_join`, `path_parent`, or
  `path_extension` with a managed `Buffer`
- **THEN** behavior and signatures remain unchanged from the canonical baseline
- **AND** examples that use Buffer workflows continue to compile

### Requirement: Standard-library status errors SHALL expose stable categories and messages
Fallible stdlib APIs SHALL expose stable numeric status categories through `std::status` while keeping source-level `Result<T, i64>` shapes compatible.

#### Scenario: A program distinguishes common error causes
- **WHEN** a stdlib helper fails because of an invalid argument, missing path, permission denial, unsupported platform feature, parse failure, timeout, interrupted operation, buffer-too-small output, or I/O failure
- **THEN** the returned error identifies the corresponding `STATUS_*` category when the cause is known
- **AND** host failures that cannot be distinguished portably return `STATUS_UNKNOWN`

#### Scenario: A program copies a generic diagnostic
- **WHEN** a program imports `std::status`
- **THEN** it can copy a stable status name or message into a managed `Buffer`
- **AND** existing `std::error` assertion helpers remain a separate module

### Requirement: JSON utilities SHALL use managed handles for parsing, querying, building, and serialization
The standard library SHALL provide `std::json` helpers using runtime-owned `JsonDoc` handles, document-owned `JsonValue` handles, and managed `Buffer` output.

#### Scenario: A program parses, queries, builds, and closes JSON
- **WHEN** a program imports `std::json`
- **THEN** it can parse JSON text or buffer bytes, query object/array/scalar values, build new JSON values, serialize into a `Buffer`, and close document handles
- **AND** parse failures and incompatible scalar reads return stable status categories and copyable diagnostics
- **AND** streaming JSON, JSON5, schema validation, and dynamic Sengoo object mapping still require a future OpenSpec update

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

### Requirement: Later process and data-format extensions SHALL remain gated by explicit follow-up design

This requirement SHALL keep later process and data-format extensions gated by
explicit follow-up design. The `stdlib-next-usability-wave` change satisfies the
follow-up design gate for handle-based `std::json` and shell-free process
command/output helpers. This
`stdlib-breadth-mainstream` change separately satisfies the gate for bounded
TOML/INI config helpers, regex helpers, encoding/compression helpers, and the
stabilized HTTP/network APIs described here. Later expansions beyond these
accepted APIs SHALL NOT be added opportunistically.

#### Scenario: This breadth wave proposes additional data helpers

- **WHEN** implementation agents add TOML, INI, regex, encoding, compression, or stabilized HTTP/network helpers
- **THEN** they follow this change's API shape, portability constraints, resource constraints, lifecycle semantics, and tests
- **AND** they do not add streaming parsers, schema validation, shell execution, background tasks, async network execution, or implicit TLS guarantees without another OpenSpec update

#### Scenario: A future phase proposes additional process or data-format features

- **WHEN** a future implementation needs streaming JSON, JSON5, schema validation, dynamic Sengoo object mapping, implicit shell commands, pipes, background tasks, signals, cancellation, async process execution, async network execution, or unbounded watchers
- **THEN** it first updates OpenSpec with API shape, portability constraints, security constraints, lifecycle semantics, and tests

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
The standard library examples SHALL include a first-class `std::collections` example for the currently supported runtime-backed `Vec<T>`, `HashMap<K, V>`, iterator helpers, copied-text lists, and string-key scalar maps.

#### Scenario: A user looks for collection examples
- **WHEN** a user opens `examples/stdlib`
- **THEN** the catalog includes a runnable `std::collections` example
- **AND** the example distinguishes scalar runtime-backed collections from copied-text list and string-key scalar map helpers

#### Scenario: A later phase proposes additional string or generic containers
- **WHEN** a future implementation needs borrowed string storage, owned-string collection returns, arbitrary generic string values, or string-value maps beyond the copied-key scalar maps accepted here
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

#### Scenario: Advanced directory operations remain explicitly scoped
- **WHEN** implementation agents add recursive traversal or portable metadata
  reads
- **THEN** they follow the accepted traversal-handle and metadata requirements
- **AND** recursive deletion, glob matching, symlink-following traversal,
  owned-string entry returns, and arbitrary persistent list APIs still require a
  future OpenSpec update

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

#### Scenario: Advanced process management remains explicitly scoped
- **WHEN** implementation agents add dynamic argv, stream capture, cwd/env
  overrides, or timeout helpers
- **THEN** they follow the accepted command/output requirements
- **AND** implicit shell commands, pipes, background handles, signals,
  cancellation, and async execution still require a future OpenSpec update

### Requirement: Process utilities SHALL support dynamic shell-free command execution and output capture
The standard library SHALL provide runtime-owned process command and output handles for dynamic literal argv, optional cwd/environment overrides, stdout/stderr capture, and timeouts while preserving fixed-arity `process_run*` helpers.

#### Scenario: A program controls and captures a child process
- **WHEN** a program creates a process command, appends literal arguments, configures cwd/env/capture/timeout options, and runs it
- **THEN** it receives a `ProcessOutput` handle on successful child creation
- **AND** it can read the child exit code, timeout flag, and captured stdout/stderr bytes through managed `Buffer` helpers
- **AND** a nonzero child exit code remains a successful process output result

### Requirement: Stdlib runtime C bridges SHALL be split and linked as a bundle
The C runtime bridge SHALL keep `runtime.c` as the anchor/core source while large domain bridges live in sibling C files compiled and linked as one runtime source bundle.

#### Scenario: A runtime sibling source changes
- **WHEN** a sibling runtime source changes
- **THEN** runtime source fingerprinting detects the change
- **AND** cached native build/run artifacts are invalidated or relinked

#### Scenario: A program uses split runtime symbols
- **WHEN** a program imports stdlib APIs implemented outside `runtime.c`
- **THEN** native build, run, reflection native linking, and stdlib runtime tests include the required sibling runtime object files

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

### Requirement: Assertion helpers SHALL migrate to std::assert without breaking std::error

The standard library SHALL expose `std::assert` as the primary assertion-helper
module while preserving existing `std::error` assertion helper behavior during a
compatibility period.

#### Scenario: A new example imports assertions

- **WHEN** a new stdlib or tutorial example needs assertion helpers
- **THEN** it imports `std::assert`
- **AND** `std::error` examples that already use assertion helpers continue to compile

#### Scenario: Runtime status helpers are needed

- **WHEN** a program needs stable runtime status names or messages
- **THEN** it imports `std::status`
- **AND** `std::error` is not extended with runtime status-classification responsibilities

### Requirement: String and formatting utilities SHALL support mainstream text workflows

The standard library SHALL provide bounded `std::string` and `std::fmt` helpers
for construction, split, join, trim, replace, primitive formatting, and explicit
byte/Unicode boundary behavior.

#### Scenario: A program formats primitive values

- **WHEN** a program formats integers, booleans, status names, or byte-stable text
- **THEN** it can produce output through an owned `String` when available or a managed `Buffer` otherwise
- **AND** formatting failure returns a status-category error rather than panicking

#### Scenario: Unicode-sensitive behavior is requested

- **WHEN** a future implementation needs grapheme clusters, normalization, locale-aware formatting, or collation
- **THEN** it first updates OpenSpec with API shape, portability constraints, and tests

### Requirement: Regex utilities SHALL provide bounded matching and captures

The standard library SHALL provide regex compile, match, capture extraction, and
replace helpers with documented pattern/input limits and deterministic error
categories.

#### Scenario: A program extracts captures

- **WHEN** a program compiles a regex and matches text with capture groups
- **THEN** it can copy the full match and indexed or named captures into accepted text outputs
- **AND** missing captures return `STATUS_NOT_FOUND`

#### Scenario: A regex exceeds limits

- **WHEN** a pattern, input, capture count, or replacement output exceeds documented limits
- **THEN** the helper returns a stable resource or unsupported status
- **AND** it does not enter unbounded catastrophic backtracking

### Requirement: Logging and time utilities SHALL support common CLI and service output

The standard library SHALL provide `std::log` and `std::time` helpers for
level-based logging, deterministic testable sinks, monotonic durations, and
date/time formatting and parsing with explicit timezone rules.

#### Scenario: A program logs at a configured level

- **WHEN** a program emits log records below and above the configured level
- **THEN** records below the level are skipped
- **AND** records at or above the level are written to the configured supported sink

#### Scenario: A program parses a date/time string

- **WHEN** a program parses a date/time string through `std::time`
- **THEN** accepted formats, timezone assumptions, and invalid-input behavior are documented
- **AND** parse failure returns `STATUS_PARSE`

### Requirement: Filesystem, config, hash, encoding, compression, and HTTP helpers SHALL be practical and bounded

The standard library SHALL provide bounded helpers for glob, file watch support
detection, recursive copy/delete policies, TOML/INI config data, SHA-style
hashing, base64/hex encoding, gzip/zlib-class compression, and HTTP workflows
with explicit support limits.

#### Scenario: A program uses config and encoding helpers

- **WHEN** a program parses TOML or INI, hashes bytes, encodes base64 or hex, or compresses/decompresses data
- **THEN** each helper documents input/output limits and copies diagnostics where applicable
- **AND** invalid input returns `STATUS_PARSE`, `STATUS_INVALID_ARGUMENT`, or another stable category

#### Scenario: A program uses filesystem policy helpers

- **WHEN** a program glob-lists paths, recursively copies, recursively deletes, or requests file-watch behavior
- **THEN** ordering, symlink policy, overwrite/delete flags, and unsupported-platform behavior are explicit and tested

### Requirement: Network helpers SHALL stabilize the existing std::net baseline before expansion

The standard library SHALL inventory and stabilize the existing `std::net` and
HTTP runtime baseline before adding broader client or server APIs.

#### Scenario: Existing net helpers are classified

- **WHEN** implementation begins this lane
- **THEN** each existing `std::net` or HTTP helper is classified as stable public API, compatibility-only API, or internal bridge
- **AND** docs and examples use only stable public API names

#### Scenario: A network feature is unsupported

- **WHEN** TLS, DNS, bind, listen, connect, or socket behavior is unsupported on the host
- **THEN** the helper returns `STATUS_UNSUPPORTED` or a more specific stable status
- **AND** native linking does not fail because of unresolved optional network symbols

### Requirement: HTTP client helpers accept HTTPS URLs with verified TLS

The `std::http` client surface SHALL support `https://` URLs using the host
platform trust store and SHALL reject insecure certificate verification bypass in
this phase.

#### Scenario: HTTPS GET succeeds against a trusted endpoint

- **WHEN** a program calls `http_client_get("https://example.test/...", timeout_ms)`
  against a test endpoint with a certificate trusted by the host store
- **THEN** the call returns `Ok(HttpResponse)` with a readable status code
- **AND** response body copy helpers behave identically to plain HTTP

#### Scenario: Plain HTTP behavior is unchanged

- **WHEN** a program calls `http_client_get("http://...", timeout_ms)`
- **THEN** behavior matches the pre-change plain HTTP implementation
- **AND** existing realworld and runtime tests continue to pass

#### Scenario: Untrusted or hostname-mismatched certificates fail with stable status

- **WHEN** a program calls `http_client_get` with an `https://` URL whose server
  presents an untrusted or hostname-mismatched certificate
- **THEN** the call returns `Err` with a stable TLS-related `std::status` code
- **AND** the failure is observable in both native tests and `sgc test` smoke paths

#### Scenario: Unsupported schemes remain unsupported

- **WHEN** a program uses non-HTTP schemes such as `ftp://`
- **THEN** the client returns `STATUS_UNSUPPORTED` as before
- **AND** the support matrix documents HTTPS as supported subset and FTP as unsupported

### Requirement: TLS failures SHALL map to stable status categories

HTTPS client failures SHALL use the existing positive `std::status` namespace.
This change SHALL add these stable categories unless a later accepted design
replaces the table before implementation starts:

| Name | Value | Meaning |
| --- | --- | --- |
| `STATUS_TLS_CERT_INVALID` | `15` | certificate chain is untrusted, expired, malformed, or otherwise invalid |
| `STATUS_TLS_HOSTNAME_MISMATCH` | `16` | certificate is valid but does not match the requested host |
| `STATUS_TLS_HANDSHAKE` | `17` | TLS negotiation failed after a backend was available |
| `STATUS_TLS_UNAVAILABLE` | `18` | TLS backend or trust-store capability is unavailable on the host |

#### Scenario: TLS categories are observable through std::status

- **WHEN** HTTPS fails for an untrusted certificate, hostname mismatch,
  handshake-level failure, or unavailable backend
- **THEN** `Result.error` uses the matching `STATUS_TLS_*` category when the cause
  is known
- **AND** `status_name_copy` and `status_message_copy` return stable names and
  human-readable messages for the new categories
- **AND** failures that cannot be distinguished portably return
  `STATUS_TLS_HANDSHAKE` rather than inventing unstable host-specific values

### Requirement: HTTPS scope is documented for production hosts

Sengoo SHALL document TLS client prerequisites (trust store, platform backends, and
CI skip policy) in the realworld support matrix and package README paths.

#### Scenario: Support matrix cites HTTPS proof

- **WHEN** this change archives
- **THEN** `examples/realworld/SUPPORT_MATRIX.md` moves TLS/HTTPS from Deferred to
  Supported subset with a concrete test or example path
- **AND** documented skips name the missing host capability rather than substituting
  fake TLS stubs

#### Scenario: HTTPS tests use real verification

- **WHEN** CI or local tests exercise a successful HTTPS request
- **THEN** the test endpoint certificate is trusted through a documented
  test-specific root-store or host trust setup
- **AND** tests do not pass by disabling certificate or hostname verification

#### Scenario: POSIX reference-host proof is required before archive

- **WHEN** this change reaches archive gate
- **THEN** POSIX/reference-host evidence covers trusted success, hostname
  mismatch, untrusted certificate, and HTTPS runtime roundtrip
- **OR** the support matrix records an evidenced skip that names the missing host
  capability and leaves the claim `Platform-specific`
- **AND** no archive claim relies on fake TLS stubs, `verify=false`, disabled
  hostname verification, or a plain HTTP fallback

### Requirement: Compression helpers SHALL be demand-backed and bounded

Sengoo SHALL promote compression from a deferred placeholder only when a
committed realworld fixture demonstrates a compressed JSON, log, or package
artifact workflow through public `std::compress` APIs. Compression helpers SHALL
define API shape, output ownership, resource limits, platform behavior, and
stable failure statuses before implementation.

#### Scenario: A realworld fixture proves compression demand

- **WHEN** compression support is claimed as supported or supported subset
- **THEN** `examples/realworld` contains a committed fixture that reads or writes
  compressed JSON, logs, or package artifacts through public `std::compress`
  APIs
- **AND** the fixture passes the locked package loop or records an evidenced
  platform skip
- **AND** `examples/realworld/SUPPORT_MATRIX.md` cites the fixture and does not
  leave compression as a stale deferred row

#### Scenario: One-shot compression preserves Buffer ownership

- **WHEN** a program calls public one-shot gzip-compatible compression or
  decompression helpers with managed `Buffer` inputs and outputs
- **THEN** successful helpers return the number of meaningful bytes written
- **AND** existing Buffer capacity semantics remain source-compatible
- **AND** any owned-string helper is additive and only succeeds for valid UTF-8
  output

#### Scenario: V1 gzip API names are stable

- **WHEN** compression support is promoted
- **THEN** `std::compress` exposes
  `compress_gzip_buffer(input: Buffer, input_len: i64, out: Buffer)` and
  `decompress_gzip_buffer(input: Buffer, input_len: i64, out: Buffer)` returning
  `Result<i64, i64>`
- **AND** the success value is the used output length
- **AND** failures use positive `std::status` categories

#### Scenario: Gzip metadata and checksum behavior is deterministic

- **WHEN** the same bytes are compressed on supported hosts
- **THEN** semantically irrelevant gzip metadata such as modification time and
  original filename is normalized or documented so fixture outputs remain
  deterministic
- **AND** decompression validates trailer/checksum data and rejects corrupt or
  truncated payloads with stable status categories
- **AND** the v1 supported subset is documented if it intentionally rejects
  gzip optional metadata or non-stored deflate block types

#### Scenario: Compression enforces resource limits

- **WHEN** input bytes, output bytes, decompression expansion ratio, or Buffer
  capacity exceed documented limits
- **THEN** the helper returns an error-shaped result with a stable
  `std::status` category
- **AND** the helper does not allocate unbounded memory, write past the output
  Buffer, or return a partially successful result

#### Scenario: Compression failures use stable statuses

- **WHEN** compression or decompression fails because of an invalid handle,
  invalid argument, too-small Buffer, corrupt or truncated payload, unsupported
  format/backend, allocation failure, expansion limit, or host/backend I/O
  failure
- **THEN** the public wrapper maps the failure to `STATUS_INVALID_HANDLE`,
  `STATUS_INVALID_ARGUMENT`, `STATUS_BUFFER_TOO_SMALL`, `STATUS_PARSE`,
  `STATUS_UNSUPPORTED`, `STATUS_OUT_OF_MEMORY`, `STATUS_OVERFLOW`, or
  `STATUS_IO` as appropriate
- **AND** it does not collapse known causes into a generic `1`

#### Scenario: Unsupported platforms remain link-safe

- **WHEN** the compression backend is unavailable on a host
- **THEN** `sgc check`, `sgc build`, and `sgc run` still link programs that
  import `std::compress`
- **AND** public compression helpers return `STATUS_UNSUPPORTED`
- **AND** the support matrix records the platform-specific or deferred behavior

### Requirement: Streaming data helpers SHALL require fixture-backed follow-up design

Sengoo SHALL keep streaming JSON parsing/serialization, JSON schema validation,
streaming compression handles, and dynamic data-object mapping gated behind a
later OpenSpec update with committed realworld demand, lifecycle semantics,
memory ceilings, platform behavior, and stable statuses. These helpers must not
be added opportunistically.

#### Scenario: A future fixture needs streaming JSON or schema validation

- **WHEN** a realworld workflow needs to process JSON beyond the documented
  one-shot cap, validate package/test metadata against a schema, or combine
  compressed JSON with bounded memory
- **THEN** a child change defines the parser or validator API shape, schema
  dialect where applicable, handle lifecycle, resource ceilings, output
  ownership, platform behavior, and stable statuses before implementation
- **AND** the existing one-shot JSON helpers remain source-compatible

#### Scenario: No fixture-backed demand exists

- **WHEN** implementation agents are working on stdlib thickness without a
  committed fixture that needs streaming JSON, schema validation, streaming
  compression, terminal control, file locks, long-lived watch streams, richer
  Unicode behavior, or broader network helpers
- **THEN** those features remain deferred rather than being added as ad hoc
  stdlib surface area
