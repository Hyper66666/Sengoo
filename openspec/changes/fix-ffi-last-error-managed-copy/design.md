# Managed diagnostic copy design

The old managed wrapper calls sengoo_ffi_buffer_ptr and
sengoo_ffi_buffer_len before sengoo_ffi_last_error_copy. Each getter clears
the global FFI diagnostic state, so the final raw copy observes no error.

Add sengoo_ffi_last_error_copy_to_buffer(handle:i64)->i64. The provider uses
sengoo_ffi_buffer_from_handle, which resolves the Buffer registry generation/liveness without
clearing the diagnostic. An invalid destination uses the existing FFI
invalid-handle error. For a valid destination, call the existing raw copy with
its direct bytes/capacity fields. On success commit the copied byte count to
used_len and retain the source error code/message. On capacity failure, retain
buffer bytes and used_len and report the existing buffer error. Empty
diagnostics copy zero bytes and set used_len to zero. The operation copies
exact bytes without adding a NUL terminator, matching the raw API.

The SG wrapper keeps its public signature and Result shape. It calls only the
new handle-based entry before testing the returned count. The private C
declaration has fixed C ABI, one signed-i64 handle parameter and signed-i64
result. Borrow checking may classify that exact external declaration as a
non-retaining input; no pointer result or generic name-only exception follows.

The existing error state is global rather than thread-local. This change
addresses the deterministic single-context ordering defect; it adds no
concurrency guarantee. Destination storage remains owned by its existing
Buffer owner. Tests inspect registry-backed bytes directly to avoid clearing
the very error state under test.

Four required runtime scenarios are nonempty diagnostic/code preservation,
empty copy/reset of used_len, capacity failure without storage mutation, and
invalid/closed handle. Independent SG execution of the real managed wrapper
must verify the nonempty bytes/code and bounded failures with all resource
counts restored. A saved pre-fix wrapper is the negative control. Source and
tool hashes, full revision and host are recorded in named evidence.
