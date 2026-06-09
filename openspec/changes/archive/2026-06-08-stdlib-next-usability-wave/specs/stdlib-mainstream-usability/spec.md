## ADDED Requirements

### Requirement: Standard-library status errors SHALL expose stable categories and messages

Fallible standard-library APIs SHALL return documented numeric error categories
instead of collapsing unrelated failures into one generic value. Modules MAY
also expose module-specific last-error details, but generic callers SHALL be
able to compare stable categories and copy short diagnostic text into a managed
`Buffer`. Runtime status helpers SHALL live in `std::status`; the existing
`std::error` module SHALL remain the assertion-helper module used by current
examples.

The stable public category namespace SHALL use these values:

| Category | Value |
| --- | ---: |
| `STATUS_OK` | 0 |
| `STATUS_UNKNOWN` | 1 |
| `STATUS_INVALID_ARGUMENT` | 2 |
| `STATUS_INVALID_HANDLE` | 3 |
| `STATUS_BUFFER_TOO_SMALL` | 4 |
| `STATUS_NOT_FOUND` | 5 |
| `STATUS_ALREADY_EXISTS` | 6 |
| `STATUS_PERMISSION_DENIED` | 7 |
| `STATUS_UNSUPPORTED` | 8 |
| `STATUS_IO` | 9 |
| `STATUS_PARSE` | 10 |
| `STATUS_TIMEOUT` | 11 |
| `STATUS_INTERRUPTED` | 12 |
| `STATUS_OVERFLOW` | 13 |
| `STATUS_OUT_OF_MEMORY` | 14 |

#### Scenario: A program distinguishes error causes

- **WHEN** a fallible stdlib helper fails because of an invalid argument, missing
  path, permission denial, unsupported platform feature, parse failure, timeout,
  interrupted operation, buffer-too-small output, or I/O failure
- **THEN** the returned error value identifies the corresponding stable category
- **AND** successful return shapes remain compatible with the existing
  `Result<T, i64>` convention

#### Scenario: A program copies a generic error name

- **WHEN** a program imports `std::status`
- **AND** calls an error-name or error-message helper with a known category and
  managed `Buffer`
- **THEN** the helper copies a deterministic ASCII diagnostic string
- **AND** returns an ok-shaped `Result<i64, i64>` containing the byte count

#### Scenario: Legacy generic failures remain compatible

- **WHEN** a current stdlib wrapper can infer a stable failure category from a
  runtime return value
- **THEN** it returns the corresponding `STATUS_*` value rather than the legacy
  generic `1`
- **AND** source-level function names, successful return shapes, and raw helper
  behavior remain compatible

#### Scenario: Unknown or module-specific errors remain diagnosable

- **WHEN** a stdlib module encounters a host-specific failure that cannot be
  mapped more precisely
- **THEN** it returns an unknown or module-specific category
- **AND** any available detailed message is exposed through a module last-error
  copy helper or generic message helper

#### Scenario: Raw module-specific codes are mapped for public wrappers

- **WHEN** an existing raw runtime helper exposes a negative module-specific
  error code such as an FFI invalid-argument, invalid-handle, buffer, internal,
  or allocation failure
- **THEN** new public safe wrappers map that raw code into the positive
  `std::status` category namespace for `Result.error`
- **AND** raw helpers MAY continue exposing module-specific codes when the API
  name explicitly documents raw behavior

### Requirement: Stdlib runtime C bridges SHALL be split and linked as a bundle

The C runtime bridge SHALL keep `runtime.c` as the anchor/core source while
large domain-specific stdlib bridges live in sibling C files. Native builds,
`sgc run`, reflection native linking, and runtime tests SHALL compile and link
the whole runtime source bundle.

#### Scenario: A runtime sibling source changes

- **WHEN** a sibling runtime source such as `runtime_json.c`,
  `runtime_process.c`, or `runtime_collections.c` changes
- **THEN** runtime source fingerprinting detects the change
- **AND** cached native build/run artifacts are invalidated or relinked

#### Scenario: A program uses split runtime symbols

- **WHEN** a program imports stdlib APIs implemented outside `runtime.c`
- **THEN** native build, run, and test helper linking include the required
  sibling runtime object files
- **AND** unresolved runtime symbols are not deferred to users

### Requirement: Managed Buffer helpers SHALL support composable text and byte ownership

The managed `Buffer` type SHALL remain the shared stdlib boundary for
runtime-owned text and byte output until Sengoo specifies an owned-string return
ABI. New helper APIs SHALL preserve existing `Buffer` uses while adding explicit
capacity, used-length, clear, byte-range copy, string append/copy, and UTF-8
validation behavior. `Buffer.len()` SHALL keep its existing capacity meaning;
new public helpers SHALL use `capacity()`, `used_len()`, `clear()`,
`copy_range(start, len, out)`, `copy_from_str(value)`, `append_str(value)`, and
`is_utf8()` names unless this spec is updated.

#### Scenario: Existing Buffer capacity behavior remains compatible

- **WHEN** an existing program passes `buffer.len()` as output capacity to a
  stdlib helper
- **THEN** the program continues to compile and the helper continues treating
  that value as the writable capacity

#### Scenario: A program composes Buffer writes

- **WHEN** a program creates a managed `Buffer`, clears it, copies or appends
  `&str` data, and then copies a byte range into another `Buffer`
- **THEN** each operation reports the number of meaningful bytes written
- **AND** `used_len()` reflects the meaningful bytes after copy, append, clear,
  and range-copy operations
- **AND** capacity and used length are distinguishable

#### Scenario: A program validates text returned through a Buffer

- **WHEN** a program asks whether the meaningful bytes in a `Buffer` are valid
  UTF-8
- **THEN** valid UTF-8 returns true
- **AND** invalid byte sequences return false without panicking or reading beyond
  the meaningful byte length

### Requirement: Text collections SHALL copy string data into runtime-owned storage

The standard library SHALL provide collection helpers for common text workflows
without storing borrowed `&str` references whose lifetime cannot be guaranteed.
Text list values and string map keys SHALL be copied into runtime-owned storage
on insertion, and text outputs SHALL copy into managed `Buffer` handles.

#### Scenario: A program stores and retrieves copied text

- **WHEN** a program appends text values to a text list
- **AND** later copies an element into a managed `Buffer`
- **THEN** the copied output equals the text inserted at that index
- **AND** the collection remains valid even if the original `&str` input came
  from a temporary expression

#### Scenario: A program uses string keys for scalar map values

- **WHEN** a program inserts `&str` keys into string-key maps with `i64` or
  `bool` values
- **THEN** it can test key existence, read values, replace values, remove keys,
  and free the map through safe stdlib wrappers
- **AND** key text is copied into runtime-owned storage on insertion

#### Scenario: Duplicate string keys replace existing values

- **WHEN** a program inserts the same string key more than once
- **THEN** the later insertion replaces the previous value
- **AND** the helper returns success rather than an already-exists error

#### Scenario: String-key iteration is deterministic

- **WHEN** a program iterates keys in a string-key map
- **THEN** the iteration order is deterministic by unsigned byte ordering of key
  text
- **AND** each key can be copied into a managed `Buffer`
- **AND** ordering does not apply Unicode normalization, locale collation, or
  case-folding

### Requirement: JSON utilities SHALL use managed handles for parsing, querying, building, and serialization

The standard library SHALL provide `std::json` helpers for JSON data-format
workflows using runtime-owned handles and managed `Buffer` outputs rather than
requiring arbitrary dynamic Sengoo values.

#### Scenario: A program parses and closes a JSON document

- **WHEN** a program imports `std::json`
- **AND** parses a JSON string or explicit bytes from a managed `Buffer`
- **THEN** a valid document returns an ok-shaped managed `JsonDoc` handle
- **AND** callers can explicitly close the document handle

#### Scenario: JSON parsing enforces documented resource limits

- **WHEN** a JSON parse request exceeds the implementation's documented input
  byte, nesting depth, or node-count limit
- **THEN** parsing returns an error-shaped result
- **AND** no partially valid `JsonDoc` handle is returned
- **AND** the default limits are documented in stdlib docs and examples

#### Scenario: A program queries JSON object and array values

- **WHEN** a parsed JSON document contains nested objects and arrays
- **THEN** a program can query value kind, object property existence, array
  length, array item handles, and object property handles through safe wrappers
- **AND** invalid paths, missing properties, and out-of-range indexes return
  error-shaped results rather than panicking

#### Scenario: A program reads JSON scalar values

- **WHEN** a JSON value is null, bool, string, or number
- **THEN** the program can read null/bool status, copy string bytes into a
  managed `Buffer`, read a number as `f64`, and read a number as exact `i64`
  when representable
- **AND** incompatible scalar reads return error-shaped results

#### Scenario: A program builds and serializes JSON

- **WHEN** a program constructs JSON object, array, string, number, bool, or
  null handles
- **THEN** it can serialize the resulting value into a managed `Buffer`
- **AND** the serialized bytes are valid JSON
- **AND** every managed JSON handle has an explicit close/free path

#### Scenario: Parse diagnostics are copyable

- **WHEN** JSON parsing fails
- **THEN** the parser returns an error-shaped result with a parse category
- **AND** callers can obtain the byte offset when available
- **AND** callers can copy a short parse diagnostic into a managed `Buffer`

### Requirement: Filesystem utilities SHALL expose portable metadata and deterministic recursive traversal

The standard library SHALL extend file and directory helpers with portable
metadata reads and deterministic recursive traversal. The API SHALL avoid
recursive deletion, glob matching, and symlink following by default.

#### Scenario: A program reads portable metadata

- **WHEN** a program requests metadata for an existing regular file, directory,
  or symlink
- **THEN** it can determine the path kind through a stable numeric category
- **AND** regular files expose byte length
- **AND** modification time is exposed in Unix milliseconds when supported by
  the host
- **AND** unsupported metadata fields return an unsupported error category

#### Scenario: A program walks a directory tree deterministically

- **WHEN** a program creates a recursive traversal handle for a root directory
- **THEN** repeated `next` calls copy one child path at a time into a managed
  `Buffer`
- **AND** traversal order is deterministic by sorted unsigned path bytes
- **AND** entries named `.` and `..` are never returned
- **AND** the traversal handle has an explicit close/free helper

#### Scenario: Recursive traversal is bounded and symlink-safe by default

- **WHEN** a program configures max depth for traversal
- **THEN** entries deeper than that depth are not returned
- **AND** symlink targets are not followed unless a future spec explicitly adds
  follow-symlink behavior

### Requirement: Process utilities SHALL support dynamic shell-free command execution and output capture

The standard library SHALL extend `std::process` with a runtime-owned command
builder and output handle for dynamic argv, optional cwd/environment overrides,
stdout/stderr capture, and timeouts. Existing fixed-arity `process_run*`
helpers SHALL remain source-compatible.

#### Scenario: A program runs a command with dynamic literal arguments

- **WHEN** a program creates a command from an executable path and appends any
  number of `&str` arguments
- **AND** runs the command
- **THEN** each argument is passed as one literal child argv entry
- **AND** the runtime does not interpret spaces or shell metacharacters as shell
  syntax

#### Scenario: A program captures stdout and stderr

- **WHEN** a command is configured to capture stdout and stderr
- **AND** the child exits normally
- **THEN** the run returns an ok-shaped `ProcessOutput` handle
- **AND** callers can read the child exit code
- **AND** callers can copy captured stdout and stderr bytes into managed
  `Buffer` handles
- **AND** a nonzero child exit code remains a successful process output result

#### Scenario: A program overrides cwd and environment

- **WHEN** a command configures a working directory, sets environment variables,
  removes environment variables, or clears inherited environment variables
- **THEN** the child process observes those settings when the host supports them
- **AND** unsupported operations return an unsupported error category
- **AND** docs warn that clearing inherited environment can remove `PATH` and
  make executable lookup fail unless callers pass an absolute executable path
  or restore required variables

#### Scenario: A command timeout is reported explicitly

- **WHEN** a command is configured with a timeout in milliseconds
- **AND** the child does not complete before the timeout
- **THEN** the runtime attempts portable child termination where available
- **AND** `ProcessOutput.timed_out()` or the error result reports timeout
  explicitly
- **AND** `ProcessOutput.exit_code()` returns a `STATUS_TIMEOUT` error unless
  the host can provide a final child exit code after termination
- **AND** any captured partial output that is safely available can be copied
  through the normal output helpers

#### Scenario: Process handles are closed explicitly

- **WHEN** a program creates command or process-output handles
- **THEN** each handle has a safe close/free helper
- **AND** using a closed or invalid handle returns an error-shaped result rather
  than reading freed storage

## MODIFIED Requirements

### Requirement: Process and data-format usability SHALL be gated by explicit follow-up design

Process execution and JSON-like data-format helpers SHALL NOT be added
opportunistically. This `stdlib-next-usability-wave` change satisfies the
follow-up design gate for handle-based `std::json` and dynamic shell-free
process command/output helpers. Any later process or data-format expansion
beyond this change still requires a new OpenSpec update.

#### Scenario: This wave proposes JSON helpers

- **WHEN** implementation agents add `std::json`
- **THEN** they follow this change's handle-based API shape, portability
  constraints, security/resource constraints, and tests
- **AND** they do not add streaming JSON, JSON5, schema validation, or dynamic
  Sengoo object mapping without another OpenSpec update

#### Scenario: This wave proposes additional process features

- **WHEN** implementation agents add dynamic argv, capture, cwd/env override,
  or timeout helpers
- **THEN** they follow this change's shell-free command/output handle design,
  lifecycle semantics, portability constraints, security constraints, and tests
- **AND** they do not add implicit shell commands, pipes, background tasks,
  signals, cancellation, or async process execution without another OpenSpec
  update

### Requirement: Collection ergonomics SHALL document currently supported runtime-backed shapes

The standard library examples SHALL include first-class `std::collections`
coverage for the currently supported runtime-backed scalar shapes plus this
change's copied-text list and string-key scalar map shapes.

#### Scenario: A user looks for collection examples

- **WHEN** a user opens `examples/stdlib`
- **THEN** the catalog includes runnable `std::collections` examples
- **AND** the examples distinguish scalar runtime-backed collections from
  copied-text list and string-key scalar map helpers
- **AND** they do not imply unsupported arbitrary generic string-value
  containers

#### Scenario: A later phase proposes additional string or generic containers

- **WHEN** a future implementation needs `HashMap<&str, &str>`,
  `HashMap<&str, Buffer>`, arbitrary generic string values, borrowed string
  storage, or owned-string collection returns
- **THEN** it first updates OpenSpec with the required value, string,
  byte-slice, and ownership model

### Requirement: Directory utilities SHALL support deterministic non-recursive listing

The standard library SHALL continue to provide `std::dir` helpers for counting
immediate directory entries and copying one entry name into a managed `Buffer`.
This change separately permits deterministic recursive traversal through a
persistent traversal handle.

#### Scenario: A program counts immediate directory entries

- **WHEN** a Sengoo program imports `std::dir`
- **AND** calls `dir_entry_count(path)` on a readable directory
- **THEN** the helper returns an ok-shaped `Result<i64, i64>` containing the
  number of immediate child entries
- **AND** the count excludes `.` and `..`

#### Scenario: A program copies a deterministic entry name

- **WHEN** a directory contains entries named `b.txt` and `a.txt`
- **AND** a program calls `dir_entry_name(path, 0, buffer)`
- **THEN** the helper copies `a.txt` into the managed `Buffer`
- **AND** returns an ok-shaped `Result<i64, i64>` with the number of bytes
  copied
- **AND** it does not append a NUL terminator

#### Scenario: Listing order is stable across host iteration order

- **WHEN** a directory contains multiple entries
- **THEN** `dir_entry_name` indexes entries after sorting names by unsigned
  byte order

#### Scenario: Invalid listing requests are rejected

- **WHEN** a program lists a non-directory path, uses a negative or out-of-range
  index, or provides an output `Buffer` that is too small
- **THEN** the helper returns an error-shaped `Result<i64, i64>`

#### Scenario: Advanced directory operations remain explicitly scoped

- **WHEN** implementation agents add recursive traversal or portable metadata
  reads
- **THEN** they follow this change's traversal-handle and metadata requirements
- **AND** recursive deletion, glob matching, symlink-following traversal,
  owned-string entry returns, and arbitrary persistent list APIs still require a
  future OpenSpec update

### Requirement: Process utilities SHALL support synchronous shell-free child execution

The standard library SHALL continue to provide `std::process` helpers for
running a child executable directly with zero through three explicit string
arguments. This change separately permits dynamic shell-free command builders
and output capture while preserving the fixed-arity helpers.

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

- **WHEN** the executable path is empty, a used raw argument pointer is missing,
  the argument count is outside zero through three, startup fails, waiting
  fails, or the child does not exit normally
- **THEN** the helper returns an error-shaped result

#### Scenario: Advanced process management remains explicitly scoped

- **WHEN** implementation agents add dynamic argv, stream capture, cwd/env
  overrides, or timeout helpers
- **THEN** they follow this change's command/output requirements
- **AND** implicit shell commands, pipes, background handles, signals,
  cancellation, and async execution still require a future OpenSpec update
