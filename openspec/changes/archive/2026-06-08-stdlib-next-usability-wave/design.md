## Overview

This wave should make the standard library feel more like a practical scripting
and tooling surface without forcing a premature language-level string ownership
change. The design keeps the current `Buffer` and handle-oriented runtime model,
adds clearer cross-module contracts, and defines higher-level APIs that can be
implemented in independent agent lanes.

## Design Principles

1. Preserve the existing public helpers.
   Current examples using `std::file`, `std::dir`, `std::io`,
   `std::process`, `std::args`, `std::path`, reflection wrappers, and scalar
   collections must keep compiling.

2. Keep text ownership explicit.
   Runtime-produced text and bytes continue to copy into managed `Buffer`
   handles or runtime-owned handles with explicit `close`/`free` methods.
   Public safe wrappers should avoid exposing raw pointer choreography except in
   existing `_raw` escape hatches.

3. Make errors inspectable.
   Fallible stdlib APIs should return documented numeric categories and expose
   module-specific detail strings through `Buffer` copy helpers when available.
   Nonzero child exit codes are still process results, not stdlib startup errors.

4. Prefer deterministic behavior.
   Directory and map iteration that crosses runtime boundaries must document its
   order. Filesystem traversal should sort path bytes to avoid host iteration
   differences in tests and examples.

5. Keep shell behavior explicit.
   Process helpers must pass literal argv entries to child processes. They must
   not invoke a shell internally or reinterpret metacharacters.

## API Shape

### Status Taxonomy

Current stdlib wrappers that can infer a stable failure cause should return a
specific `std::status` category instead of the legacy generic `error: 1`.
Successful return shapes and function names stay source-compatible. Wrappers
that cannot distinguish the host failure reason map to `STATUS_UNKNOWN`.

`std::status` should provide stable status categories such as invalid argument,
invalid handle, buffer too small, not found, already exists, permission denied,
unsupported, I/O failure, parse failure, timeout, interrupted, overflow, out of
memory, and unknown. `std::error` remains the existing assertion-helper module;
it must not be repurposed for runtime status categories.

`Result<T, i64>.error` should carry a `std::status` category for new and
migrated public stdlib wrappers. Module wrappers may keep module-specific
last-error helpers for detailed diagnostics, but generic code should be able to
compare categories and copy a short name/message for diagnostics.

Stable public category numbers:

| Name | Value | Notes |
| --- | ---: | --- |
| `STATUS_OK` | 0 | Success placeholder; failed results must not use it. |
| `STATUS_UNKNOWN` | 1 | Legacy generic failure and unmapped host failures. |
| `STATUS_INVALID_ARGUMENT` | 2 | Bad argument value, null raw pointer, invalid index, or malformed option. |
| `STATUS_INVALID_HANDLE` | 3 | Runtime-owned handle is zero, closed, or not recognized. |
| `STATUS_BUFFER_TOO_SMALL` | 4 | Caller-provided output buffer cannot hold the requested bytes. |
| `STATUS_NOT_FOUND` | 5 | Missing path, key, property, executable, or resource. |
| `STATUS_ALREADY_EXISTS` | 6 | Destination/key/resource exists when replacement was not requested. |
| `STATUS_PERMISSION_DENIED` | 7 | Host permission failure. |
| `STATUS_UNSUPPORTED` | 8 | Platform or runtime does not support the requested operation. |
| `STATUS_IO` | 9 | Host I/O or wait failure that is not mapped more specifically. |
| `STATUS_PARSE` | 10 | Text/data-format parse failure. |
| `STATUS_TIMEOUT` | 11 | Operation exceeded a configured timeout. |
| `STATUS_INTERRUPTED` | 12 | Operation was interrupted by the host. |
| `STATUS_OVERFLOW` | 13 | Numeric overflow or unrepresentable exact conversion. |
| `STATUS_OUT_OF_MEMORY` | 14 | Allocation failure. |

Existing module-specific raw error codes, including negative FFI/runtime codes,
remain implementation details for last-error helpers during migration. Public
safe wrappers added by this change should map those raw details to the positive
`std::status` namespace in `Result.error`; raw helpers may continue returning
legacy codes when their name already advertises raw or module-specific behavior.

### Managed Buffer and Text

The current `Buffer` type in `ffi.sg` remains the shared runtime-owned byte
container. Existing `Buffer.len()` behavior must stay compatible and continue
to mean writable capacity. New public names should be:

- `Buffer.capacity()` as an alias for writable capacity;
- `Buffer.used_len()` for meaningful initialized bytes;
- `Buffer.clear()` to reset used length to zero without freeing capacity;
- `Buffer.copy_range(start, len, out)` for explicit byte-range copy;
- `Buffer.copy_from_str(value)` to replace meaningful bytes with `value`;
- `Buffer.append_str(value)` to append `value`;
- `Buffer.is_utf8()` to validate meaningful bytes.

These helpers distinguish capacity from initialized byte length, support
clearing, copying explicit byte ranges, validating UTF-8, and appending/copying
`&str` content without requiring owned-string returns.

When a stdlib function writes into a `Buffer`, the returned byte count remains
the primary success result. Implementations may also update a tracked used length
for callers that compose multiple operations.

### Text Collections

String-key maps and text lists should copy text into runtime-owned collection
storage at insertion time. That avoids dangling borrowed `&str` references and
keeps lifecycle explicit through collection `free`/iterator `free` methods.
Output text should copy into caller-provided `Buffer` handles.

The first implementation only needs common shapes:

- ordered text list append/get/set/remove/iter operations;
- string-key to `i64` and string-key to `bool` maps;
- deterministic key iteration and key copy-out.

Duplicate key insertion should replace the existing value and return success.
Key comparison and deterministic ordering are byte-based over UTF-8 bytes when
the key is valid UTF-8; no Unicode normalization, locale collation, or
case-folding is applied. Full arbitrary generic string values can wait for a
later owned-string ABI.

### JSON Handles

`std::json` should use handles instead of dynamic Sengoo values:

- `JsonDoc` owns a parsed or constructed document and must be closed;
- `JsonValue`/selector handles may be document-owned lightweight views;
- object/array access returns handles or typed scalar results;
- string and serialized output copies into `Buffer`;
- parse errors expose category, byte offset where available, and a copied
  message.

Numbers should support `f64` reads and exact `i64` reads when representable.
Streaming JSON, comments, JSON5, schema validation, and arbitrary language
object conversion are out of scope.

JSON parsers/builders should enforce documented resource limits for input byte
length, nesting depth, and total node count. Defaults may be implementation
defined, but examples and docs must state them. Failed parses must not return a
partially valid document handle; only successful parse/build calls produce
handles that callers must close.

Type-incompatible scalar reads should report `STATUS_INVALID_ARGUMENT` through
the same last-error path as other JSON query failures.

### Runtime Bridge Bundle

`tools/stdlib/runtime.c` remains the anchor/core runtime source for discovery
and metadata compatibility, but large domain bridges are split into sibling C
files. Shared status constants, handle conversion helpers, and managed Buffer
copy helpers live in `runtime_shared.h`.

Native builds, `sgc run`, reflection native linking, and stdlib runtime tests
compile and link the full runtime source bundle. The runtime fingerprint used
for cache keys includes every source in the bundle so a change to any sibling
runtime file invalidates cached runtime objects and native link artifacts.

### Filesystem Metadata and Recursive Traversal

Metadata helpers should expose portable fields first: file/directory/symlink
kind, byte length for regular files, and modification time in Unix
milliseconds when the host can provide it. Unsupported fields should fail with a
documented unsupported error category instead of fabricating values.

Recursive traversal should be a persistent runtime handle:

- create with root path and options such as max depth and symlink following;
- `next(buffer)` copies one relative or absolute path per call;
- iteration order is deterministic by sorted byte path;
- `close` releases runtime storage;
- symlinks are not followed by default.

Recursive deletion and glob matching remain out of scope for this wave.

### Process Command and Output

The existing fixed-arity `process_run*` helpers remain. New APIs should add a
runtime-owned command builder and output handle:

- command construction from an executable path;
- dynamic literal argv entries;
- optional cwd override;
- optional env set/remove/clear operations;
- inherited streams by default, opt-in stdout/stderr capture;
- timeout in milliseconds;
- blocking run that returns a `ProcessOutput` handle;
- output readers for exit code/timed-out status/stdout/stderr lengths and copy
  helpers;
- explicit `close` methods for command/output handles.

Startup failure, invalid arguments, unsupported cwd/env operations, and wait
failure are stdlib errors. A child exit code, including nonzero, is an ordinary
process output result. Timeout behavior should terminate the child process when
portable termination is available and expose `timed_out == true`; platform
limitations must be documented.

After timeout, `ProcessOutput.exit_code()` should return an error-shaped result
with `STATUS_TIMEOUT` unless the host can provide a final child exit code after
termination. `ProcessOutput.timed_out()` remains the authoritative timeout
indicator. Captured partial output may be readable when safely available. Env
clear helpers should document that clearing inherited environment can remove
`PATH` and make executable lookup fail unless callers pass an absolute
executable path or restore required variables.

## Implementation Lanes

- Lane A: cross-module `std::status` taxonomy and `Buffer`/text helpers.
- Lane B: text collections built on copied runtime-owned text.
- Lane C: `std::json` handle API and examples.
- Lane D: filesystem metadata and recursive traversal.
- Lane E: process command/output capture and timeout.
- Lane F: wiring, docs, LSP, examples, and OpenSpec validation.

Each lane can land behind tests while preserving existing stdlib behavior.

## Risks

- Buffer compatibility: `Buffer.len()` is already used as capacity in existing
  modules. New used-length helpers must not silently break that convention.
- Resource leaks: JSON, traversal, command, output, and text collection handles
  need close/free coverage in examples and tests.
- Windows/POSIX differences: process timeouts, symlinks, and metadata fields
  must report unsupported/unknown categories where behavior cannot be portable.
- Runtime file growth: JSON, traversal, process builders, status mapping, and
  text collections should not all be appended to an ever-larger
  `tools/stdlib/runtime.c` if the project already has or can cheaply add
  domain-specific runtime bridge files.
- Scope creep: async I/O, shells, globbing, and dynamic language object mapping
  are tempting but should remain outside this change unless the spec is updated.

## Verification Strategy

Implementation agents should add focused regression tests per lane, then run the
project's stdlib-facing verification:

- OpenSpec validation for this change and all specs.
- Rust/compiler test suites that cover stdlib import expansion and runtime ABI.
- `sgc check`, `sgc build`, and `sgc run` for new `examples/stdlib` examples.
- `sglsp` stdlib symbol/signature tests for every new public module/helper.
- Platform-specific smoke tests for filesystem and process behavior where the
  host supports it.
