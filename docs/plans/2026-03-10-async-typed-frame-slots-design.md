# Async Typed Frame Slots Design

## Goal
Support non-`i64` single-slot values across async suspend/resume without changing the current async runtime frame ABI.

## Current State
- Async helpers still use an `i64` frame slot ABI:
  - `sengoo_async_frame_alloc(i64 slot_count)`
  - `sengoo_async_frame_store(i64 handle, i64 offset, i64 value)`
  - `sengoo_async_frame_load(i64 handle, i64 offset) -> i64`
- `__start`, `__poll`, and `__result` in [compiler/src/mir/async_lowering.rs](/D:/Sengoo/compiler/src/mir/async_lowering.rs) currently assume async parameters, spilled locals, child future handles, and final results can all flow through raw `i64`.
- That is now acceptable for `i64`-like values, but not for `bool`, `i32`, references/pointers, or other non-aggregate scalar values.

## Decision
Keep the runtime ABI unchanged and add compiler-side encode/decode for a restricted set of frame-storable types.

Supported in this batch:
- `bool`
- `i8`, `i16`, `i32`, `i64`
- `ref`
- `ptr`
- `Future<T>` handles

Explicitly rejected in this batch when they must cross an await point:
- `tuple`
- `struct`
- `array`
- `enum`
- any other aggregate / multi-slot MIR type

## Approach
1. Introduce a small frame-slot classification helper in async lowering.
2. Encode supported values to `i64` before storing them in the frame.
3. Decode frame-loaded `i64` values back to the original MIR type before resuming execution or returning from `__result`.
4. Reject unsupported aggregate values in async helper synthesis with a dedicated async compile error.

## Error Model
Add a dedicated top-level compile error for async lowering:
- `CompileError::AsyncUnsupportedType { ty, reason }`

Reason string for this batch:
- `aggregate types (tuple/struct/array/enum) cannot cross await points yet`

This error should be used when:
- an async parameter must be stored in the frame but its MIR type is unsupported
- an async result must be read from the frame but its MIR type is unsupported
- a live local crossing an await point has an unsupported MIR type

## Codegen Implications
The current `Instruction::Cast` lowering is not sufficient for all needed conversions.

Required support:
- LLVM backend:
  - `bool -> i64` via `zext`
  - `i8/i16/i32 -> i64` via `sext`
  - `i64 -> i8/i16/i32` via `trunc`
  - `ptr/ref/future-handle -> i64` via `ptrtoint`
  - `i64 -> ptr/ref/future-handle` via `inttoptr`
- JIT backend:
  - same semantics as LLVM, not `bitcast` approximations

## Testing
Compiler tests:
- `async_bool_local_survives_await`
- `async_i32_local_survives_await`
- `async_ref_local_survives_await`
- `async_tuple_local_across_await_rejected`
- `async_struct_local_across_await_rejected`

Native runtime tests:
- `async_native_runtime_preserves_bool_across_resume`
- `async_native_runtime_preserves_i32_across_resume`
- `async_native_runtime_preserves_ref_across_resume`

## Non-Goals
- aggregate frame layout
- tuple/struct/array/enum values across await
- async block execution
- runtime ABI redesign
- borrow/lifetime extensions for references
