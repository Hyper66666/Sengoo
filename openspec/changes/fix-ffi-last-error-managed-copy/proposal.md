# Preserve FFI errors when copying into a managed Buffer

## Why
The managed ffi_last_error_copy wrapper asks for a Buffer pointer and capacity
before copying the diagnostic. Both getters clear the current FFI error, so a
valid nonempty diagnostic becomes an empty successful copy. A managed error
copy must inspect the destination without destroying the source error first.

## What Changes
Add a fixed C handle-based copy entry that resolves the registered Buffer
without invoking clearing getters. Reuse the existing raw byte-copy operation,
commit used_len only on success, and point the existing SG managed wrapper at
that entry. Preserve the raw pointer/capacity API and existing failure codes.
Add one exact non-retaining handle-input contract for this new runtime
declaration; it does not authorize a raw projected output.

## Impact
The owner is the runtime/FFI implementation in tools/stdlib/runtime.c and
tools/stdlib/ffi.sg. Regression evidence covers the C provider and the public
SG wrapper separately. This child change does not complete self-hosting,
change ordinary FFI signatures, or expand async/SDK admission.
