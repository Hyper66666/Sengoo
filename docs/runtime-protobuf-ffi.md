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
- `u64 sengoo_proto_user_event_decode_open(const u8* input_ptr, usize input_len)`
- `i64 sengoo_proto_user_event_decoded_id(u64 handle)`
- `i64 sengoo_proto_user_event_decoded_ts(u64 handle)`
- `i64 sengoo_proto_user_event_decoded_name_len(u64 handle)`
- `i64 sengoo_proto_user_event_decoded_name_copy(u64 handle, u8* buffer, usize capacity)`
- `i32 sengoo_proto_user_event_decoded_close(u64 handle)`
- `i32 sengoo_proto_last_error_code()`
- `i64 sengoo_proto_last_error_len()`
- `i64 sengoo_proto_last_error_copy(u8* buffer, usize capacity)`
- `i32 sengoo_proto_last_error_clear()`

## Sengoo stdlib wrapper

`import std::proto` preloads the protobuf wrapper and its `std::ffi` dependency.
Encode uses a managed `Buffer` handle instead of raw output pointer/capacity
arguments. Managed decode returns a runtime-owned `ProtoDecodedUserEvent`
handle, so field access no longer requires caller-owned scalar output pointers.
Pass the encoded byte length explicitly because a newly allocated `Buffer`
retains its capacity:

```sg
import std::proto;

def main() -> i64 {
    let event = proto_user_event(7, "alice", 42);
    let out = ffi_buffer_new(128);
    if out.is_err() {
        0
    } else {
        let buffer = out.unwrap_or(Buffer { handle: 0 });
        let written = proto_user_event_encode(event, buffer);
        let name = ffi_buffer_new(32).unwrap_or(Buffer { handle: 0 });
        let decoded = proto_user_event_decode(buffer, written.unwrap_or(0));
        let decoded_event = decoded.unwrap_or(ProtoDecodedUserEvent { handle: 0 });
        let name_len = decoded_event.name_copy(name).unwrap_or(0);
        let result = decoded_event.id() + decoded_event.ts() + name_len;
        decoded_event.close();
        name.free();
        buffer.free();
        result
    }
}
```

The low-level `sengoo_proto_user_event_decode` and
`proto_user_event_decode_raw` entrypoints remain available for explicit pointer
handoff.

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
