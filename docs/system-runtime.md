# Native system coordination

Development APIs require Sengoo 0.2.1-dev.0 and the native backend. These generic
helpers contain no application policy. Import them with `import std::system;`.

`file_lock(path) -> Result<FileLock,i64>` creates or opens a permanent coordination
file and immediately acquires an exclusive OS lock. Contention returns status
11. `file_lock_mode(path, exclusive)` additionally permits shared locks. The
parent directory must exist. The owning, non-Send/non-Sync `FileLock` releases
its lock in Drop; `close(&mut self)` also releases it and clears its handle.
Neither operation deletes the file. Windows denies deletion while the handle
exists. POSIX locks are advisory, and every participant must preserve the lock
file: unlinking it can create two separately locked inodes. These are local
filesystem coordination primitives; network-filesystem lock semantics depend
on the mounted filesystem and are not qualified by local tests.

`canonical_path_string(path) -> Result<String,i64>` resolves an existing file or
directory through the OS and returns its absolute UTF-8 target path. Windows
uses a normalized file handle path, resolving junctions and case aliases;
POSIX uses `realpath` and preserves case-sensitive names. Paths are at most
32 KiB of UTF-8, and invalid UTF-8, embedded NUL or a missing target fails.
Canonicalization is an observation at call time: it does not lock a directory
against later rename or symlink replacement.

On Windows, `credential_write(target,value) -> Result<bool,i64>`,
`credential_read(target) -> Result<String,i64>` and
`credential_delete(target) -> Result<bool,i64>` use current-user Generic
Credentials. Targets contain 1–1024 UTF-8 bytes; values contain 0–2560 UTF-8
bytes. Both reject embedded NUL and invalid UTF-8. Write replaces that target's
value, persisting it on the local machine for the same OS user. Read returns
an independent owned String; missing read returns status 5. Delete returns true
when removed and false when already absent. Native temporary value copies are
cleared before free; caller-owned Sengoo/Python strings retain their normal
memory lifecycle. Helpers never enumerate credentials or print values.
All credential calls return status 8 on POSIX; they create no plaintext fallback.

`tools/sgc/tests/system_runtime.rs` compiles a real OS probe, verifies concurrent
process exclusion and persistent identity, and tests the public Sengoo wrapper
and Drop. The Windows fixture creates a unique credential containing only a
disposable test string, then deletes it. Its path checks create an isolated
junction and a differently cased alias. Linux/macOS behavior requires execution
on those hosts before claiming platform qualification.

Windows x64 native and AddressSanitizer checks passed with clang 19.1.7 on
2026-11-04, based on `d46b3f6cd8d6e770734f98edc57aa67cb931e26a` plus working-tree
changes. These are local development results; qualification must rerun at the
eventual committed candidate. The public wrapper test additionally verifies
String, FileLock and opaque-handle counts return to their pre-call baselines.
