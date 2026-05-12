# runtime_proto ABI Note

Driver: `runtime/src/reflect/runtime_proto.rs`

Status codes:

- `0`: success
- `-2801`: invalid argument
- `-2802`: parse error
- `-2803`: truncated input/output
- `-2899`: internal error

Extern symbols:

- `sengoo_proto_last_error_code() -> i32`
- `sengoo_proto_last_error_len() -> i64`
- `sengoo_proto_last_error_copy(buffer: *mut u8, capacity: usize) -> i64`
- `sengoo_proto_last_error_clear() -> i32`
- `sengoo_proto_user_event_encode(id: u32, name: *const u8, ts: u64, out_buffer: *mut u8, out_capacity: usize) -> i64`
- `sengoo_proto_user_event_decode(input_ptr: *const u8, input_len: usize, out_id: *mut u32, out_name_buffer: *mut u8, out_name_capacity: usize, out_ts: *mut u64) -> i64`

Ownership:

- The runtime never owns caller buffers.
- Encode writes to caller-provided output buffer.
- Decode writes scalar/string fields into caller-provided output pointers.

Sengoo wrapper note:

The MVP wrapper exposes the already implemented `UserEvent` shape only.
