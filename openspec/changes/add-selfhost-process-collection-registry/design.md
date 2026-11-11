# Design

## Profile and token model

The new `sengoo_sg_process_*` APIs are an unsafe experimental Windows x64 ABI1
process/DLL profile with 128 registry slots. A successful token is negative:
`-(2^62 + generation*256 + slot+1)`, generation 1..16777215, slot 0..127.
Zero alone denotes creation failure. Old `handle <= 0` invalid/status conventions
MUST NOT be applied. Valid positive explicit-context and reference opaque tokens
are rejected before payload access; those old APIs reject the new negative tokens.
Foreign negative status values and deliberately forged matching integers carry no
authority: callers must retain the authentic token and family provenance.

The root has eight control words plus 128 four-word slots: 4,160 bytes. A slot is
generation, kind, payload pointer, reserved. SG initializes a zero OS-provided root,
selects slots, checks the bounded profile, retires max-generation slots and never
resets generations. One C storage object/DLL owns the root; duplicate copies and
DLL unload/reload are different lifetimes, not a globally unique capability.
No shutdown resets the root while stale handles could exist.

Kind 1 denotes a canonical RawVec mapping payload, kind 2 an owning word-block.
The unsafe raw new/get/take interface requires authentic, sufficiently accessible,
nonoverlapping payloads of that family. RawVec wrappers record a mapping magic
and the actual positive handle returned by the existing SG context core. Arithmetic
conversion of an unrelated integer cannot manufacture a mapping. Word-blocks
record their own magic, length and owned payload words. Same payload pointer
published twice/across families violates the caller's unique-owner obligation.

## Synchronization and ownership

OS SRWLOCK leaves serialize root initialization and slot operations. Get returns
an unpinned raw pointer after unlocking, so callers must serialize payload access
against close/mutation. Metadata stress is not concurrent-payload safety. Take
detaches an entry and unlocks before any Drop callback, vector free or memory
release. Real callback-reentry tests must create/take another registry entry while
the outer vector is being freed; callbacks never run under the registry lock.

RawVec uses the already-qualified context/core algorithm and a private process
context address. Its positive inner handles never escape as process tokens.
Word-block arithmetic and ownership, mapping construction, cleanup, failure
unwinding and callback decisions are SG. C has no generation/kind dispatch or
slot search. Direct raw registry calls remain unsafe implementation plumbing;
they do not admit a provider or create safe borrowed references.

## Deployment

The C storage leaf provides only two zero-storage addresses and OS lock/unlock.
Existing `collections_memory.c` supplies memory/callback primitives unchanged.
The old qualified SG linker rejects a real `.bss` input. The explicit development
SDK branch therefore keeps real `.bss`, state, auxiliary records and unwind in a
minimal source-bound DLL with pinned OS imports and short import library. Qualified
SG-linked executables consume that import library and all SG algorithm objects.
Nothing strips state or silently substitutes an unqualified linker. Two fresh
directories compare actual SG objects, adapted C memory/fixture objects, DLL,
import library and PE bytes, then run with empty PATH.

The profile is not the process-global replacement for String, Buffer, legacy/raw
Map/Vec, Rc/Arc, futures, scheduler TLS, AsyncFile or network/reflection maps. The
actual inventory and target global-address proof remain historical development
evidence; a later source-bound migration must address each domain independently.
