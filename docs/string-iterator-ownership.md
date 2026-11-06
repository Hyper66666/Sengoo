# String snapshot iterator ownership

The development toolchain containing this change automatically releases
`StringBytesIter`, `StringCharsIter` and `StringSplitIter` when their owning
Sengoo scope exits. This applies to early returns and fully consumed iterators.
Previously a function could create one of these iterators, return without an
explicit `free`, and leave the native snapshot allocated. A regression first
reproduced that retention before Drop was added.

Iteration remains byte, Unicode scalar, and independently owned String items,
respectively. Each iterator keeps a snapshot; dropping its original String
does not invalidate that snapshot. Explicit `free()` is still supported and
borrows the wrapper. Repeated explicit free before metadata reuse succeeds;
`next` then reports exhaustion, and automatic Drop remains safe. A stale native
token after its slot is reused is invalid and cannot close the new owner.

Native entry points check generation and domain rather than dereferencing a
caller-provided integer as a pointer. Byte/scalar iterators and split iterators
use separate token domains. Metadata slots are reused, so retained metadata
tracks peak concurrency instead of the number of completed calls. The existing
one-handle layout, C entry-point names and signatures, edition 2026 and runtime
ABI 1 remain compatible. Handles are opaque and must not be cast to pointers.

`cargo test -p sgc --test string_iter_ownership` passed 2/2 on Windows x64 on
2026-11-04. Sengoo integration covers early return, exhaustion, repeated explicit
free followed by Drop, and 10,000 cycles with zero live iterators. The independent
native probe checks foreign/cross-domain/stale tokens, snapshot independence and
50,000 cycles with bounded metadata. The existing String iterator associated
Item generic-bound compatibility regression also passed 1/1; byte, scalar and
owned String item types remain unchanged.

The native probe also passed clang 19.1.7 AddressSanitizer on this host. Windows
LeakSanitizer is not claimed: live-allocation and slot counters verify release
separately. IPC, terminal fullscreen and stopped-reader stdin probes passed the
same address-safety instrumentation. Absolute Windows ASan log paths require a
quoted value, for example `log_path='D:\Sengoo\target\string-iter-asan'`.

This is working-tree evidence based on
`d46b3f6cd8d6e770734f98edc57aa67cb931e26a`, using clang 19.1.7 and local MSVC.
Candidate verification must rerun at its committed revision. Other platforms
and published release binaries are not certified by this Windows run.
