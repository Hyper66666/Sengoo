# Sengoo Standard Library Sources

The MVP standard library is split into small source modules so compiler tests,
runtime wrappers, and examples can depend on only the surfaces they need.

- `option.sg`: generic `Option<T>`, generic constructors (`option_some`,
  `option_none_with`), i64/bool convenience constructors, bool/i64 unwrap and
  expect helpers, and i64 map helpers.
- `result.sg`: generic `Result<T, E>`, generic constructors (`result_ok_with`,
  `result_err_with`), i64 and `Result<bool, i64>` convenience constructors, and
  bool/i64 unwrap, map, and projection helpers.
- `collections.sg`: runtime-backed `Vec<T>`, `HashMap<K, V>`, transitional
  `HashSet<T>` for i64/bool/string keys, deterministic
  `BTreeMap<String, ...>` / `BTreeMap<i64, ...>` and matching sets, iterators,
  i64/bool collection mutators, `Rc<i64>`/`Rc<bool>`/`Rc<String>` shared
  ownership with `RcValue` generic construction, copied-text lists, and
  string-key maps for scalar i64/bool values.
- `string.sg`: borrowed `&str` helpers (`str_len`, equality, search, repeat) plus owned `String` (`string_new`, `string_from_str`, `string_from_buffer`, borrow via `as_str`, `clone`, `push_str`, `push_i64`, `push_char`, `clear`, `copy_to_buffer`, `drop`, `eq`) backed by `runtime_string.c`.
- `strconv.sg`: runtime-backed decimal `i64`, `f32`, and `f64` conversion helpers for parsing `&str` or Buffer bytes and formatting values into managed `Buffer` handles.
- `math.sg`: integer helpers (`abs_i64`, `min_i64`, `max_i64`, `sign_i64`, `clamp_i64`, `gcd_i64`, `lcm_i64`, `pow_i64`), trait-bound `numeric_abs/min/max/clamp` for every supported integer and f32/f64 family, `i64` overflow helpers (`wrapping_*`, `checked_*`, `saturating_*`), checked `i64` conversions to fixed-width integer types, plus f32/f64 helpers (`abs_*`, `min_*`, `max_*`, `sqrt_*`, `pow_*`, `exp_*`, `ln_*`, `floor_*`, `ceil_*`, `round_*`, `sin_*`, `cos_*`, `tan_*`, `is_nan_*`, `is_finite_*`, `is_infinite_*`).
- `error.sg`: compatibility assertion helpers (prefer `assert.sg` for new code).
- `assert.sg`: primary assertion helpers for boolean, i64, string, and f64 checks.
- `fmt.sg`: primitive formatting into managed `Buffer` handles via `strconv` and `status`.
- `regex.sg`: bounded regex compile/match helpers (`runtime_breadth.c`).
- `log.sg`: level-based logging with deterministic test sink output.
- `config.sg`: bounded INI/TOML subset parse/get helpers.
- `hash.sg`: SHA-256 hex digest helpers.
- `encoding.sg`: base64 and hex encode helpers for byte buffers.
- `compress.sg`: deterministic one-shot gzip/gunzip Buffer helpers backed by a bounded stored-deflate subset.
- `fs.sg`: glob listing, file copy/remove wrappers, and file-watch support detection.
- `http.sg`: stable HTTP client surface over the existing runtime HTTP ABI.
- `status.sg`: stable stdlib status categories plus category name/message copy helpers for fallible runtime APIs.
- `json.sg`: runtime-backed JSON parse/query/build/serialize helpers using document/value handles and managed `Buffer` outputs.
- `file.sg`: runtime-backed file helpers for existence checks, byte length, metadata, string write/append, removal, copy/move with explicit overwrite selection, and reading into managed `Buffer` handles.
- `dir.sg`: runtime-backed directory helpers for existence checks, idempotent single-directory creation, recursive creation, deterministic listing, bounded recursive walking, and empty-directory removal.
- `io.sg`: runtime-backed synchronous standard I/O helpers for Buffer-backed stdin reads, exact stdout/stderr writes, and stream flushing.
- `env.sg`: runtime-backed environment helpers for variable presence, variable length/copy into managed `Buffer` handles, platform checks, and conventional exit-code selection.
- `time.sg`: runtime-backed clock and sleep helpers plus UTC `YYYY-MM-DDTHH:MM:SSZ` format/parse helpers.
- `random.sg`: runtime-backed deterministic pseudo-random helpers for seeding, non-negative i64 values, half-open i64 ranges, and booleans.
- `path.sg`: runtime-backed path helpers for platform separator discovery, conservative absolute checks, joining, parent/file-name/stem/extension extraction, and lexical normalization into managed `Buffer` handles.
- `process.sg`: runtime-backed process metadata helpers, synchronous shell-free fixed-arity child execution, and command-builder handles for dynamic argv, cwd/env overrides, output capture, and timeouts.
- `args.sg`: runtime-backed command-line argument helpers for user argument count, existence checks, byte lengths, and managed `Buffer` copy. The executable/source path is not exposed as argument index `0`.
- `db.sg`, `ffi.sg`, `lua54.sg`, `net.sg`, `proto.sg`: Sengoo-side wrappers over the runtime reflection drivers.
- `runtime.c`: anchor/core C runtime support used by stdlib/runtime smoke
  paths. Large domain bridges live in sibling sources:
  `runtime_breadth.c`, `runtime_collections.c`, `runtime_json.c`,
  `runtime_process.c`, and `runtime_string.c`, with shared declarations in
  `runtime_shared.h`.

## Source Imports

`sgc check`, `sgc build`, and `sgc run` understand source-level stdlib imports
and preload the requested module before compiling:

```sg
import std::collections;

def main() -> i64 {
    let values = vec_new_i64();
    values.push(41);
    values.get(0).unwrap_or(0) + 1
}
```

For modules that use `Option<T>` or `Result<T, E>`, `sgc` also preloads the
current source dependencies (`option.sg` and `result.sg`) automatically.
Reflection modules can declare their own source dependencies as well. `import
std::args`, `import std::collections`, `import std::compress`,
`import std::config`, `import std::db`, `import std::dir`,
`import std::encoding`, `import std::env`, `import std::file`,
`import std::fmt`, `import std::fs`, `import std::hash`,
`import std::http`, `import std::io`, `import std::json`,
`import std::log`, `import std::lua54`, `import std::net`,
`import std::path`, `import std::process`, `import std::proto`,
`import std::regex`, `import std::status`, `import std::strconv`, and
`import std::time` preload the needed source dependencies so managed `Buffer`
helpers and stable status categories are available for output payloads and
error-shaped results.

## Status and Buffer Helpers

`std::status` exposes stable numeric categories for fallible stdlib APIs:
`STATUS_OK()` is `0`, `STATUS_UNKNOWN()` is the legacy generic failure `1`,
and specific categories such as `STATUS_INVALID_ARGUMENT()`,
`STATUS_INVALID_HANDLE()`, `STATUS_BUFFER_TOO_SMALL()`,
`STATUS_NOT_FOUND()`, `STATUS_ALREADY_EXISTS()`,
`STATUS_PERMISSION_DENIED()`, `STATUS_UNSUPPORTED()`, `STATUS_IO()`,
`STATUS_PARSE()`, `STATUS_TIMEOUT()`, `STATUS_INTERRUPTED()`,
`STATUS_OVERFLOW()`, `STATUS_OUT_OF_MEMORY()`, and `STATUS_CANCELED()` use
the positive namespace specified by OpenSpec. `status_name_copy(code, buffer)` and
`status_message_copy(code, buffer)` copy deterministic ASCII diagnostics into a
managed `Buffer`. `status_from_raw_ffi(code)` maps existing negative FFI raw
codes, negative `-STATUS_*` runtime returns, and positive status categories
into the public positive namespace. Current stdlib fallible wrappers use this
taxonomy for `Result.error`; host failures that cannot be classified portably
map to `STATUS_UNKNOWN()`.

Network helpers keep raw `net_last_error()` compatibility accessors, but
fallible `std::net` and `std::http` wrappers map raw network/runtime bench
errors into this taxonomy before filling `Result.error`.

`Buffer.len()` remains the legacy capacity helper. New composable helpers make
that explicit: `capacity()`, `used_len()`, `clear()`,
`copy_range(start, len, out)`, `copy_from_str(value)`, `append_str(value)`, and
`is_utf8()`. These helpers operate on meaningful bytes tracked by the Buffer
itself; stdlib functions that write through `buffer.ptr()` still report their
meaningful byte count through their `Result` return value.

## Assertion And Test Helpers

`std::assert` is the primary assertion module for new tests. Assertion failures
emit schema-v1 JSON envelopes with helper name, message, callsite file/line, and
expected/actual payloads when the helper has those values. `sgc test --format
json` keeps its existing top-level fields stable and omits optional `coverage`
and per-test `parameters` fields unless those features are active. `sgc test`
discovers legacy file-level tests, top-level `def test_*` functions, and
`#[test]` functions in test files that do not define their own `main`; the
runner strips the tooling-only `#[test]` marker from generated harnesses before
invoking the normal compiler path. Generated function-test harnesses call
top-level `setup()` before each test case and `teardown()` after each test case
when those fixture functions are present. `#[case("label", ARG)]` lines
immediately before a test function generate one case per argument, with JSON
`parameters` entries for the case label and `arg0`. `sgc test --coverage`
compiles statement-line probes into test binaries, registers all executable
source probes, and aggregates only runtime hits into the v1 text and JSON
coverage summary. Uncalled functions and untaken multi-line branches are not
reported as covered. The `sengoo_coverage_register` / `sengoo_coverage_hit`
runtime hooks are toolchain-internal; ordinary builds emit no calls to them.

## Collection Helpers

`std::collections` provides an ABI-versioned, descriptor-backed `Vec<T>` for
arbitrary concrete element types. Its owning surface covers push/pop,
borrowed get, set/insert/remove, length/empty checks, clear, and automatic
element Drop. Borrowing `iter()` yields `&T`, owning `into_iter()` yields moved
`T`, and a live borrowed element or iterator blocks mutations that could move
storage.
The existing scalar `Vec<T>` and `HashMap<K, V>` helpers remain compatible for
i64/bool combinations, alongside runtime-owned text shapes for common tooling
workloads. Generic `VecDeque<T>` shares the descriptor-backed RawVec core and
supports borrowed front/back, push/pop at both ends, clear, and automatic Drop.
Transitional `VecDeque<i64>` and `VecDeque<bool>` support `push_front`,
`push_back`, `pop_front`, `pop_back`, `front`, `back`, `len`, `clear`, and
automatic scope-exit `Drop` over the same i64 vector runtime; manual `free()`
remains source-compatible. `Vec<String>` supports
owned-string `push`, `set`, `insert`, cloned reads, transfer removal, cloned
iteration, and consuming iterator `collect()` through the existing
string-vector runtime. `HashMap<String, i64>`, `HashMap<String, bool>`, and
`HashMap<String, String>` are transition spellings over the copied-key string
map runtimes: they copy `&str` keys on insert, replace existing values for
duplicate keys, and expose deterministic key iteration by unsigned byte
ordering, including consuming `count`, `skip`, and bounded `take` on owned-key
iterators, plus `collect()` into `Vec<String>`. The same sorted runtime now
backs explicit `BTreeMap<String, i64/bool/String>` and `BTreeSet<String>`
transition types, whose iteration order is independent of insertion order.
`BTreeMap<i64, i64>`, `BTreeMap<i64, bool>`, and `BTreeSet<i64>` use a
separate sorted integer runtime rather than the hash-map slot order. They
support insertion and replacement, lookup, removal, length/clear, automatic
`Drop`, and deterministic ascending key iteration (including negative keys).
`TextList`
copies inserted `&str` values and can copy elements back into a managed
`Buffer` with `get_copy`, `remove_copy`, and iterator `next_copy`.
`StringMapI64` and `StringMapBool` remain source-compatible aliases for the
same copied-key scalar map family. Runtime-backed Vec i64 iterators support
single-step `map_with`/`filter_with` plus consuming `count`, `sum`, `skip`,
`take`, `collect`, and transitional `enumerate()`; bool vector iterators
support `map_with`/`filter_with` plus consuming `count`, `skip`, `take`,
`collect`, and transitional `enumerate()`. Runtime-backed i64/bool map
iterators also expose transitional `enumerate()` over yielded values.
Transitional `HashSet<i64>`/`HashSet<bool>` iterators enumerate set keys and
can consume `skip`, `take`, `count`, or `collect`, while string-key maps and
sets expose copied/owned key iteration plus counting, bounded `take`, and
`collect()` into `Vec<String>`. The verified scalar/string set handles now
release automatically on scope exit; manual `free()` remains compatible.
`Vec<String>` iterators can consume `count`, `skip`, `take`, or
collect cloned strings into a new `Vec<String>`.
Key ordering is byte based only; Unicode
normalization, locale collation, and case folding are not applied. Generic
`HashMap<K,V>` and `HashSet<T>` use compiler-generated Hash/Eq and move/Drop
callbacks for arbitrary concrete keys and values. Generic `BTreeMap<K,V>` and
`BTreeSet<T>` use Ord callbacks, sorted insertion, deterministic borrowed key
iteration, and the same exact key/value Drop contract.
Owning generic iterators expose lazy `skip`, `take`, `map`, `filter`, and
`enumerate` state machines plus consuming `count`, `fold`, and `collect`.
Numeric item types implement `SumValue`, so `sum()` preserves the item type and
returns that type's zero identity for an empty iterator. Use
`collect_hashset()` for an explicit set sink or
`collect_hashmap(projector)` with a projector returning `MapEntry<K,V>`; these
explicit targets avoid return-type-only generic method inference and move
accepted keys and values into the generic collection core.

`Rc<T>` is a single-threaded shared-ownership handle for the verified payloads
`i64`, `bool`, and `String`. Use `rc_new_i64`, `rc_new_bool`, and
`rc_new_string` directly, or write generic helpers with `T: RcValue` and
`value.rc()` when the payload is one of those supported types. Arbitrary
user-defined `Rc<T>` storage remains deferred until the runtime has a stable
value layout and drop ABI for user values.

## String Conversion Helpers

`std::strconv` provides deterministic decimal `i64` conversion. `strconv_parse_i64`
parses a normal `&str`, while `strconv_parse_i64_raw` and
`strconv_parse_i64_buffer(buffer, len)` parse explicit byte ranges so callers
can consume data returned by `std::args`, `std::file`, or `std::io`.
`strconv_format_i64(value, buffer)` writes base-10 ASCII into a managed Buffer
and returns the byte count; it does not append a NUL terminator. Parsing accepts
optional ASCII whitespace and a leading sign, rejects non-whitespace trailing
characters, and reports overflow as an error-shaped `Result`. `strconv_parse_f32`,
`strconv_parse_f64`, their Buffer variants, and `strconv_format_f32`/
`strconv_format_f64` provide fixed-precision decimal float conversion over the
same status taxonomy. Radix selection, locale-specific formatting, arbitrary
precision values, and owned-string returns remain deferred.

## JSON Helpers

`std::json` parses JSON text into `JsonDoc` handles and exposes `JsonValue`
handles for object, array, string, number, bool, and null values. `json_parse`
accepts `&str`, while `json_parse_buffer(buffer, input_len)` parses explicit
Buffer bytes. Object and array queries return error-shaped `Result` values for
missing keys, out-of-range indexes, and type mismatches. Strings and serialized
documents copy into managed `Buffer` handles. Numbers can be read as `f64`, or
as exact `i64` when representable.

Builders create object, array, string, number, bool, and null values inside a
document. `JsonDoc.serialize(buffer)` writes compact valid JSON, and
`JsonDoc.close()` releases runtime-owned handles. Parser diagnostics are
available through `json_last_error_code()`, `json_last_error_offset()`, and
`json_last_error_copy(buffer)`. The current runtime enforces conservative
limits of 1 MiB input bytes, 64 nesting levels, and 4096 nodes; failed parses
return no closeable partial document handle.

## Compression Helpers

`std::compress` provides one-shot gzip Buffer helpers for CLI/config artifacts.
`compress_gzip_buffer(input, input_len, out)` accepts at most 1 MiB of input and
emits deterministic gzip bytes using stored deflate blocks, `mtime=0`, no
original filename/comment/extra fields, and `OS=255`. The compressed output is
limited by the output Buffer capacity.

`decompress_gzip_buffer(input, input_len, out)` validates gzip magic, method,
stored block length/complement pairs, CRC32, and ISIZE. It accepts v1 stored
gzip inputs up to 1,048,679 bytes, which is the largest stream this encoder can
produce for a 1 MiB input. Decompressed output is capped at
`min(out.capacity(), 4 MiB, 4x compressed input length)`. Corrupt or truncated
data returns `STATUS_PARSE()`, unsupported gzip metadata or non-stored deflate
blocks return `STATUS_UNSUPPORTED()`, oversized input returns
`STATUS_OVERFLOW()`, and small output Buffers return
`STATUS_BUFFER_TOO_SMALL()`.

## Directory Helpers

`std::dir` covers the portable directory operations needed by small scripts and
tooling: `dir_exists(path)`, `dir_create(path)`, `dir_create_all(path)`,
`dir_entry_count(path)`, `dir_entry_name(path, index, buffer)`, and
`dir_remove(path)`. Creation is idempotent when the target directory already
exists. Listing is non-recursive, excludes `.` and `..`, sorts entry names by
unsigned byte order for deterministic indexes, copies one child name into a
managed `Buffer`, and returns the byte count without appending a NUL terminator.
`dir_walk(root, max_depth)` creates a traversal handle that copies sorted
relative child paths into a managed `Buffer` one at a time. The walk excludes
`.` and `..`, does not follow symlinks, and stops recursing past `max_depth`.
`DirWalk.close()` releases the traversal handle. `dir_remove` only removes
empty directories; recursive tree deletion, glob matching, owned-string entry
returns, and watch APIs remain deferred.

## File Helpers

`std::file` provides binary `file_copy(source, destination, overwrite)` and
host-rename `file_move(source, destination, overwrite)` helpers alongside its
basic read, write, append, length, existence, and removal operations. Copy
returns the transferred byte count, while move returns an ok-shaped boolean
when the host rename succeeds. Existing destinations are rejected unless
callers explicitly opt into replacement, and copy rejects source aliases that
already refer to the destination file. Metadata helpers expose
`file_kind(path)`, `file_size(path)`, and `file_modified_unix_ms(path)`.
`PATH_KIND_FILE()`, `PATH_KIND_DIR()`, and `PATH_KIND_SYMLINK()` are stable
path-kind values; unsupported fields return `STATUS_UNSUPPORTED()`. Recursive
directory transfer, cross-filesystem move fallback, metadata-preservation
guarantees, atomic-copy claims, progress callbacks, and async file I/O remain
deferred.

## Standard I/O Helpers

`std::io` provides synchronous process stream helpers. `io_stdin_read(buffer)`
copies up to `buffer.len()` bytes from stdin, and `io_stdin_read_line(buffer)`
copies up to the buffer capacity or through one newline; EOF without bytes is a
successful `0` byte read. `io_stdout_write(data)` and `io_stderr_write(data)`
write exactly the provided string bytes without appending newlines, while
`io_stdout_flush()` and `io_stderr_flush()` expose fallible stream flushing.
Async I/O, terminal control, file descriptor APIs, and owned-string stdin
helpers remain deferred.

## Path Helpers

`std::path` treats both `/` and `\` as separators. `path_separator()` returns
the host-preferred separator byte, and `path_is_absolute` recognizes Unix roots,
Windows drive roots such as `C:/tmp`, and UNC-like leading double separators.
String-producing helpers (`path_join`, `path_parent`, `path_file_name`,
`path_stem`, `path_extension`, and `path_normalize`) write into managed
`Buffer` handles and return `Result<i64, i64>` with the byte count on success.
`path_join` treats an absolute right-hand side as a replacement. `path_normalize`
is lexical only: it removes duplicate separators, `.` segments, and simple
`..` segments without touching the filesystem or resolving symlinks.

## Process Helpers

`std::process` exposes the current process ID and current working directory in
a portable subset. `process_current_dir_len()` reports the byte length of the
working directory, and `process_current_dir_copy(buffer)` copies it into a
managed `Buffer` and returns the copied byte count. `process_exit_code(success,
failure_code)` maps a boolean success value to `0` or the caller-provided
failure code. `process_run(executable)` and `process_run_1` through
`process_run_3` start the requested executable directly, inherit the current
standard streams, block until completion, and return the normal child exit
code. The runtime does not invoke a shell internally: shell metacharacters stay
inside literal child arguments unless the caller explicitly selects a shell
executable.

For dynamic commands, `process_command(executable)` returns a `ProcessCommand`
handle. Callers can append literal argv entries with `arg`, set `cwd`, configure
environment edits with `env_clear`, `env_set`, and `env_remove`, enable
`capture_stdout`/`capture_stderr`, set `timeout_ms`, and call `run()` to obtain
a `ProcessOutput` handle. Nonzero child exit codes are still successful process
outputs; `exit_code()` reports them. If a timeout kills the child,
`timed_out()` is true, `exit_code()` returns `STATUS_TIMEOUT()`, and any
already-captured partial stdout/stderr remains copyable. `env_clear()` removes
inherited variables such as `PATH`, so callers should pass an absolute
executable path or set the needed environment explicitly. `ProcessCommand.close`
and `ProcessOutput.close` release runtime-owned handles. Shell pipelines,
background handles, signals, cancellation, stdin piping, and async execution
remain deferred; command-line argument reads live in `std::args`.

## Argument Helpers

`std::args` exposes the current program's user-supplied trailing arguments.
`args_len()` excludes the executable or source path, so `sgc run main.sg --
alpha beta` and a native binary invoked as `main alpha beta` both report a count
of `2`. `arg_exists(index)` checks that user-argument index, `arg_len(index)`
returns its byte length, and `arg_copy(index, buffer)` copies bytes into a
managed `Buffer` and returns the copied byte count. Invalid indices return an
error-shaped `Result`. Command execution remains out of scope; this module only
reads the current process argument vector.

## Reflection Wrappers

The reflection wrapper modules are thin Sengoo-side surfaces over existing
runtime drivers. A shared `sengoo_stdlib_str_ptr` helper bridges Sengoo `&str`
values to the existing raw-pointer driver calls.

- `db.sg`: wraps `runtime/src/reflect/runtime_db.rs`. Lifecycle: `db_open`/`db_open_raw` returns `Db`, then call `Db.close`; query results use `DbResult.close`. Error, column-name, and cell copy helpers accept managed `Buffer` handles. Example: `examples/reflection/db_open_query.sg`.
- `ffi.sg`: wraps `runtime/src/reflect/runtime_ffi.rs`. Lifecycle: `ffi_open`/`ffi_open_raw` returns `CLib`, callbacks use `CallbackToken.unbind`, buffers use `Buffer.free`. Error copy and buffer-to-buffer copy helpers accept managed `Buffer` handles. Fixed-arity `call_i64_0` through `call_i64_4` helpers cover common C calls without raw argument/result pointers; object constructors and methods have matching helpers. Example: `examples/reflection/ffi_load_call.sg`.
- `lua54.sg`: wraps `runtime/src/reflect/runtime_lua54.rs`. Lifecycle: `lua54_open`/`lua54_open_raw` returns `Lua54`, then call `Lua54.close`. Error copy helpers accept managed `Buffer` handles, and `call_i64_0` through `call_i64_4` cover common calls without raw pointer slots. Native Lua 5.4 availability is runtime/feature-gated, so examples may exercise the diagnostic path when Lua is unavailable. Example: `examples/reflection/lua54_eval.sg`.
- `proto.sg`: wraps `runtime/src/reflect/runtime_proto.rs` for the currently implemented `ProtoUserEvent` encode/decode shape. `proto_user_event` accepts a normal `&str` name, `proto_user_event_encode` writes into a managed `Buffer`, and `proto_user_event_decode(buffer, input_len)` returns a managed `ProtoDecodedUserEvent` handle with field readers plus `close`. Raw decode/output helpers remain available for explicit pointer handoff. Example: `examples/reflection/proto_encode_decode.sg`.
- `net.sg`: wraps the public `runtime/src/net.rs` TCP/UDP/HTTP client/server/WS surface and `runtime/src/reflect/runtime_net_bench.rs`. Safe `&str` helpers cover hosts, URLs, text payloads, server routes, and required-header middleware; managed `Buffer` helpers cover receive/body/error/bench output; `_raw` helpers remain for explicit pointer/buffer handoff. Lifecycle: every nonzero handle is closed by its matching `close` method/function. In native `sgc` stdlib builds where the Rust network runtime is not linked, fallback C symbols return stable unsupported or invalid-handle statuses rather than leaving optional network symbols unresolved. Examples: `examples/reflection/net_tcp_echo.sg`, `examples/reflection/net_http_server.sg`.

Current source-level limitation: Sengoo FFI now accepts immutable `&str` C-string
parameters, and the reflection wrappers expose normal string helpers for common
paths. Managed `Buffer` handles cover FFI byte payloads, DB/Lua/FFI diagnostics,
DB result copies, protobuf encode output, and network receive/body/error/bench
output. Protobuf decoded fields are available through runtime-owned handles,
and common fixed-arity FFI/Lua calls no longer require raw pointer slots.
Dynamic-arity FFI/Lua calls still use raw `i64` pointer values until typed
slice/buffer/out-parameter support lands.
