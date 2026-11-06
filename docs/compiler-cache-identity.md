# Compiler content and cache reuse

Build, run, frontend-session and generic-instance cache JSON records now include
`compiler_identity`, the SHA-256 of the running `sgc` executable. Every persisted
cache loader verifies it before returning metadata to workset planning, codegen,
incremental linking or artifact recovery. A same-version local compiler rebuild
therefore invalidates prior compiler artifacts even if its Git revision has not
changed. Existing identity-free cache files safely miss and are regenerated;
project source, manifests and lockfiles are unaffected.

Identity is computed once per compiler process. Linux reads `/proc/self/exe` to
retain the running inode when the on-disk executable is replaced. Other hosts
use the current executable path. Read/hash failure disables loading cached
metadata and reports an error when saving. Compiler paths, timestamps and file
sizes are not substituted for content. Copying identical executable contents
to a different path preserves the identity and permits normal reuse.

`tools/sgc/tests/compiler_cache_identity.rs` exercises actual compiler invocations:
build once, copy the unchanged compiler and observe a hit, append a harmless
executable overlay without changing its version, then verify frontend and
codegen rebuild and all persisted records receive a new identity. On macOS the
modified fixture is signed ad hoc before execution. Cache unit tests also reject
missing/foreign identities for build, run and generic metadata. Windows x64
execution passed on 2026-11-04; other hosts require their own execution evidence.
