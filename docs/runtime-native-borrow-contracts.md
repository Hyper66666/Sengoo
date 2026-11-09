# Audited native observation contracts

The experimental legacy-parameter fix uses an explicit table in
`compiler/src/mir/lowering/native_borrow_contracts.rs`. These native functions
observe or copy inputs without retaining or releasing them; constructors return
independent owned storage and accessors return scalars. There is no wildcard
`sengoo_` rule and no dependency on application wrapper names.

| Native symbol | Observed positions (zero based) | Inspected implementation |
| --- | --- | --- |
| `sengoo_string_eq`, `sengoo_string_compare` | 0, 1 | `runtime_string.c`: resolve, compare lengths/bytes, return scalar |
| `sengoo_string_len`, `sengoo_string_is_empty` | 0 | `runtime_string.c`: read resolved length |
| `sengoo_string_copy_to_buffer` | 0, 1 | `runtime_string.c`: copy to initialized managed output |
| `sengoo_string_from_buffer`, `sengoo_string_from_utf8` | 0 | `runtime_string.c`: validate then allocate an independent String copy |
| `sengoo_ffi_buffer_len`, `sengoo_ffi_buffer_capacity`, `sengoo_ffi_buffer_used_len`, `sengoo_ffi_buffer_is_utf8` | 0 | `runtime.c`: resolve and read counters or validate bytes |
| `sengoo_ffi_buffer_copy_in`, `sengoo_ffi_buffer_copy_out` | 0, 1 | `runtime.c`: bounds-check and memcpy, retain neither pointer |
| `sengoo_ffi_buffer_copy_range` | 0, 3 | `runtime.c`: checked memmove between managed buffers |
| `sengoo_ffi_buffer_get_u8` | 0 | `runtime.c`: read an initialized byte |
| `sengoo_ffi_buffer_commit_used_len`, `sengoo_ffi_buffer_consume` | 0 | `runtime.c`: checked initialized-length update or memmove; retains neither owner |
| `sengoo_ffi_buffer_append`, `sengoo_ffi_buffer_index_of_bytes` | 0, 1 | `runtime.c`: checked byte copy/search; no retained pointers |
| `sengoo_arg_copy` | 1 | `runtime.c`: copy existing argument bytes into caller storage |
| `sengoo_env_var_copy`, `sengoo_file_read_into` | 0, 1 | `runtime.c`: synchronous copy/read; close/free temporary native storage before returning |
| `sengoo_net_stream_read` | 0, 1 | `runtime/src/net/byte_stream.rs`: drain synchronized byte queue into caller buffer; pointer not retained |
| `sengoo_string_from_bytes_copy` | 0 | `runtime_string.c`: validate and copy into independent String allocation |
| `sengoo_hash_sha256` | 0, 2 | `runtime_breadth.c`: synchronous digest and managed output copy |
| `sengoo_hash_sha256_bounded` | 0, 3 | `runtime_breadth.c`: bounded synchronous digest and output copy |

Applying a contract additionally requires a declared `extern "C"` function with
the exact native arity and all `i64` parameters/return. Ordinary source functions
with the same name are analyzed from their own bodies; duplicate identities do
not infer observation. Declared C types and native ABI compatibility remain
independently checked. Linking a replacement under a reserved runtime ABI symbol
cannot preserve the supported runtime contract and is outside this toolchain.

Unknown externs, direct raw-handle returns, alias storage and handle-based owner
reconstruction remain conservative transfers. A comparison may observe handle
fields, while returning/storing those identities cannot establish a borrow.
Borrowed pointer results and unavailable definitions do not gain new contracts.
`sengoo_ffi_buffer_ptr` has a separate projected-output contract, not an ordinary
observation contract: its returned pointer borrows the buffer. It may preserve
the Buffer argument only while its result is immediately consumed by a proven
noescape sink in the same complete expression. Direct return, alias storage or
unknown retaining calls remain consuming. The checker and all MIR call layers
carry this context separately from the public observation policy, and keep a
fresh owner live across later-argument failure and through the native call.

The table intentionally omits freeing functions. Further
native functions require implementation review and independent regressions.
This limited compatibility mechanism is not a complete lifetime system.
