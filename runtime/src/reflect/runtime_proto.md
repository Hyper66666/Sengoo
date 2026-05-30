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
- `sengoo_proto_user_event_decode_open(input_ptr: *const u8, input_len: usize) -> u64`
- `sengoo_proto_user_event_decoded_id(handle: u64) -> i64`
- `sengoo_proto_user_event_decoded_ts(handle: u64) -> i64`
- `sengoo_proto_user_event_decoded_name_len(handle: u64) -> i64`
- `sengoo_proto_user_event_decoded_name_copy(handle: u64, buffer: *mut u8, capacity: usize) -> i64`
- `sengoo_proto_user_event_decoded_close(handle: u64) -> i32`

Ownership:

- The runtime never owns caller-provided buffers.
- Encode writes to caller-provided output buffer.
- Raw decode writes scalar/string fields into caller-provided output pointers.
- Managed decode stores fields behind a runtime-owned handle until
  `sengoo_proto_user_event_decoded_close`.

Sengoo wrapper note:

`proto_user_event_decode(buffer, input_len)` returns a managed
`ProtoDecodedUserEvent`. The explicit payload length is required because a
newly allocated `Buffer` retains its capacity after encode.
