# Sengoo Standard Library Sources

The MVP standard library is split into small source modules so compiler tests,
runtime wrappers, and examples can depend on only the surfaces they need.

- `option.sg`: generic `Option<T>`, generic constructors (`option_some`,
  `option_none_with`), i64/bool convenience constructors, bool/i64 unwrap and
  expect helpers, and i64 map helpers.
- `result.sg`: generic `Result<T, E>`, generic constructors (`result_ok_with`,
  `result_err_with`), i64 and `Result<bool, i64>` convenience constructors, and
  bool/i64 unwrap, map, and projection helpers.
- `collections.sg`: runtime-backed `Vec<T>`, `HashMap<K, V>`, iterators, and i64/bool collection mutators. Current runtime-backed shapes cover scalar i64/bool combinations; string-key/string-value collections are deferred until Sengoo has a specified string/byte-slice ownership model.
- `string.sg`: Sengoo-side wrappers over built-in string lowering and runtime string search: `str_len`, equality, contains/prefix/suffix/index helpers, empty checks, append, and repeat.
- `strconv.sg`: runtime-backed decimal `i64` conversion helpers for parsing `&str` or Buffer bytes and formatting values into managed `Buffer` handles.
- `math.sg`: pure-Sengoo integer helpers: `abs_i64`, `min_i64`, `max_i64`, `sign_i64`, `clamp_i64`, `gcd_i64`, `lcm_i64`, and `pow_i64`.
- `error.sg`: pure-Sengoo assertion helpers for boolean, i64, string, and f64 checks.
- `file.sg`: runtime-backed file helpers for existence checks, byte length, string write/append, removal, and reading into managed `Buffer` handles.
- `dir.sg`: runtime-backed directory helpers for existence checks, idempotent single-directory creation, recursive creation, and empty-directory removal.
- `io.sg`: runtime-backed synchronous standard I/O helpers for Buffer-backed stdin reads, exact stdout/stderr writes, and stream flushing.
- `env.sg`: runtime-backed environment helpers for variable presence, variable length/copy into managed `Buffer` handles, platform checks, and conventional exit-code selection.
- `time.sg`: runtime-backed clock and sleep helpers: Unix seconds, Unix milliseconds, millisecond sleep, and elapsed/since calculations.
- `random.sg`: runtime-backed deterministic pseudo-random helpers for seeding, non-negative i64 values, half-open i64 ranges, and booleans.
- `path.sg`: runtime-backed path helpers for platform separator discovery, conservative absolute checks, joining, parent/file-name/stem/extension extraction, and lexical normalization into managed `Buffer` handles.
- `process.sg`: runtime-backed process metadata helpers for process ID, current working directory length/copy into managed `Buffer` handles, and conventional exit-code selection.
- `args.sg`: runtime-backed command-line argument helpers for user argument count, existence checks, byte lengths, and managed `Buffer` copy. The executable/source path is not exposed as argument index `0`.
- `db.sg`, `ffi.sg`, `lua54.sg`, `net.sg`, `proto.sg`: Sengoo-side wrappers over the runtime reflection drivers.
- `runtime.c`: C runtime support used by stdlib/runtime smoke paths.

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
std::args`, `import std::db`, `import std::dir`, `import std::env`, `import std::file`, `import std::io`, `import std::lua54`, `import std::net`, `import std::path`, `import std::process`, `import std::proto`, and `import std::strconv`
preload `ffi.sg` so managed `Buffer` helpers are available for output payloads.

## String Conversion Helpers

`std::strconv` provides deterministic decimal `i64` conversion. `strconv_parse_i64`
parses a normal `&str`, while `strconv_parse_i64_raw` and
`strconv_parse_i64_buffer(buffer, len)` parse explicit byte ranges so callers
can consume data returned by `std::args`, `std::file`, or `std::io`.
`strconv_format_i64(value, buffer)` writes base-10 ASCII into a managed Buffer
and returns the byte count; it does not append a NUL terminator. Parsing accepts
optional ASCII whitespace and a leading sign, rejects non-whitespace trailing
characters, and reports overflow as an error-shaped `Result`. Floats, radix
selection, locale-specific formatting, arbitrary precision values, and
owned-string returns remain deferred.

## Directory Helpers

`std::dir` covers the portable setup operations needed by small scripts and
tooling: `dir_exists(path)`, `dir_create(path)`, `dir_create_all(path)`, and
`dir_remove(path)`. Creation is idempotent when the target directory already
exists. `dir_remove` only removes empty directories; recursive tree deletion and
directory listing are deferred until Sengoo has a broader filesystem safety and
string/iterator design.

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
failure code. Command execution is intentionally deferred until Sengoo has a
specified shell-free process API; command-line argument reads live in
`std::args`.

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
- `net.sg`: wraps the public `runtime/src/net.rs` TCP/UDP/HTTP client/server/WS surface and `runtime/src/reflect/runtime_net_bench.rs`. Safe `&str` helpers cover hosts, URLs, text payloads, server routes, and required-header middleware; managed `Buffer` helpers cover receive/body/error/bench output; `_raw` helpers remain for explicit pointer/buffer handoff. Lifecycle: every nonzero handle is closed by its matching `close` method/function. Examples: `examples/reflection/net_tcp_echo.sg`, `examples/reflection/net_http_server.sg`.

Current source-level limitation: Sengoo FFI now accepts immutable `&str` C-string
parameters, and the reflection wrappers expose normal string helpers for common
paths. Managed `Buffer` handles cover FFI byte payloads, DB/Lua/FFI diagnostics,
DB result copies, protobuf encode output, and network receive/body/error/bench
output. Protobuf decoded fields are available through runtime-owned handles,
and common fixed-arity FFI/Lua calls no longer require raw pointer slots.
Dynamic-arity FFI/Lua calls still use raw `i64` pointer values until typed
slice/buffer/out-parameter support lands.
