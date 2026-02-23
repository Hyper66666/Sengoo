# Sengoo Runtime Protobuf FFI (UserEvent MVP)

This document describes the protobuf FFI chain PoC implemented for branch `feat/db-ffi-a`.

## Scope

- Wire encode/decode for a stable MVP message:

```proto
message UserEvent {
  uint32 id = 1;
  string name = 2;
  uint64 ts = 3;
}
```

- C ABI encode/decode entrypoints
- Error code + message diagnostics
- Golden-wire consistency test (matches canonical protobuf wire bytes)

## C ABI

- `i64 sengoo_proto_user_event_encode(u32 id, const u8* name, u64 ts, u8* out_buffer, usize out_capacity)`
- `i64 sengoo_proto_user_event_decode(const u8* input_ptr, usize input_len, u32* out_id, u8* out_name_buffer, usize out_name_capacity, u64* out_ts)`
- `i32 sengoo_proto_last_error_code()`
- `i64 sengoo_proto_last_error_len()`
- `i64 sengoo_proto_last_error_copy(u8* buffer, usize capacity)`
- `i32 sengoo_proto_last_error_clear()`

## Error codes

- `0`: success
- `-2801`: invalid argument
- `-2802`: parse error
- `-2803`: truncated payload
- `-2899`: internal error

## Consistency check

The test fixture validates this canonical byte sequence:

- `id=150`, `name="alice"`, `ts=9001`

Wire bytes:

`08 96 01 12 05 61 6c 69 63 65 18 a9 46`

This verifies compatibility with protobuf varint + length-delimited wire rules for the MVP schema.

