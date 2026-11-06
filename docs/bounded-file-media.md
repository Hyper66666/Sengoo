# Bounded file snapshots and media headers

These additive APIs target edition 2026 and runtime ABI 1. They require the
matching development toolchain and standard-library payload. Existing file APIs
and the published v0.2 interfaces retain their behavior. This is local Windows
x64 development evidence, not a release or qualification for other platforms.

## Owned local-file snapshot

`import std::file_snapshot;` provides:

```text
file_snapshot_available() -> bool
file_snapshot_open(path: &str, max_bytes: i64) -> Result<FileSnapshot, i64>
FileSnapshot.len() / modified_unix_ms() -> Result<i64, i64>
FileSnapshot.sha256() / identity() / path() / pinned_path() -> Result<String, i64>
FileSnapshot.copy_bytes(out: &mut Buffer) -> Result<i64, i64>
FileSnapshot.still_matches() -> Result<bool, i64>
FileSnapshot.close(&mut self) -> Result<bool, i64>
```

The owner retains immutable copied bytes and file/ancestor handles. `copy_bytes`
copies the complete sequence into an existing buffer and replaces its used
length; insufficient capacity returns `BUFFER_TOO_SMALL`. SHA-256 hashes those
same bytes, including embedded NUL. Move transfers ownership, Drop releases it,
and explicit close clears the owner before Drop. Owners are neither Send nor
Sync; keep these APIs on the single state-owner thread.

Windows accepts relative or ordinary absolute paths on local fixed volumes.
UNC/device paths, drive-relative paths, alternate data streams, reserved names,
directories and final or ancestor reparse objects are refused. The regular-file
handle excludes write/delete sharing. Ancestor handles exclude delete sharing
and remain held until close. An incompatible pre-existing file writer is
refused, rather than weakening the sharing policy. A volume GUID root obtained
from the root handle pins later lookups against changes to DOS drive aliases.

`path()` is an observed display path. `pinned_path()` is a volume GUID path for
consumers that need the retained name, including a verified executable worker;
hold the owner until the process tree has become quiescent. This API supplies
no executable trust decision, sandbox, process management or application path
authorization. Native pinned `CreateProcessW` launch has an independent fixture.

Caps are 64 MiB per snapshot, 128 MiB across owners, 32 owners and 64 guarded
directories per owner. Caller `max_bytes` must be 0 through 64 MiB. Known
oversize input is rejected before byte allocation. Reads are chunked, require
complete expected content and an additional EOF probe, then compare handle
identity, size and write metadata. Errors destroy partial allocations/handles.
Non-Windows reports `UNSUPPORTED`; it does not silently provide weaker guards.

The returned copy is frozen; its source is not a filesystem transaction.
Metadata checks do not rule out every pre-existing writable mapping or hostile
same-length modification. Ancestor guards prove delete/rename exclusion on the
tested host; they do not establish a general defense against every privileged
filesystem mutation. Windows directory write sharing is not a reliable write
exclusion primitive. The synchronous read has no input-latency or
cancellation bound. A live TUI still needs a verified cooperative or bounded
I/O-worker integration before claiming responsive attachment reads.

## Pure Sengoo media inspection

`import std::media;` provides:

```text
media_inspect(bytes: &Buffer, scan_bytes: i64, max_pixels: i64)
    -> Result<MediaInspection, i64>
media_inspect_with_limits(bytes: &Buffer, scan_bytes: i64,
    max_jpeg_segments: i64, max_dimension: i64, max_pixels: i64)
    -> Result<MediaInspection, i64>
pdf_capabilities() -> PdfCapabilities
```

`scan_bytes` is 1 through 1 MiB; explicit segment limits are 1 through 65,536;
single dimensions are 1 through 2,147,483,647; pixel limits must be positive.
The convenience form uses 4,096 JPEG segments and the format dimension maximum.
Parsing allocates no buffers from untrusted dimensions. Kind is unknown/PNG/
JPEG/PDF (0/1/2/3). Status is unknown/header/malformed/unsupported/limit
(0/1/2/3/4). Invalid caller limits return `INVALID_ARGUMENT`.

PNG checks the complete 33-byte IHDR, CRC and legal field combinations. JPEG
walks bounded segment framing and recognizes baseline, extended sequential and
progressive SOF dimensions. PDF searches for a complete `%PDF-` prefix within
the first 1,024 bytes, then inspects a bounded version line. Versions 1.0–1.7
and 2.0 are recognized; future versions are explicitly unsupported.

Every result has `full_validated = false`. Header recognition does not prove
full image/PDF validity, decoder safety, page count, encryption or EXIF
orientation. `pdf_capabilities()` reports only `header_metadata = true`;
`page_count`, `encryption_query`, `text_extract`, and `render` are false. It
executes no helper, downloads nothing and does not search PATH.

## Named local evidence

Host: Windows x64, 2026-11-04. Source HEAD
`d46b3f6cd8d6e770734f98edc57aa67cb931e26a` with recorded working-tree changes;
this HEAD alone is not a frozen candidate. Private compiler SHA-256:
`cf52f7a8ea6bdad280844a327beb9452980ee2f723324a3dd6155626f08f093b`.

Run with the configured native clang and a private Cargo target:

```text
cargo test -p sgc --test file_media -- --nocapture
```

The four named tests cover:

- `native_file_snapshot_guard_bounds_and_cleanup`: Unicode paths, exact binary
  bytes/digests/EOF, fragment reads, cap errors, forced I/O/truncation/metadata
  failures, file writer/ancestor rename exclusion, real reparse rejection, pinned
  executable launch, owner counts and warmed process-handle counts.
- `public_file_snapshot_move_drop_and_guarded_paths`: repeated owned SG reads,
  copy, move/Drop/close, failure cleanup and write admission after close.
- `pure_sengoo_media_header_classification_and_explicit_limits`: PNG CRC,
  dimensions, JPEG framing/variants, exact caller limits, PDF search boundaries,
  unsupported versions and truthful capabilities.
- `installed_file_media_consumer_uses_copied_payload_without_cargo`: copied
  compiler/runtime/stdlib in an external temporary install, both public fixtures
  built with PATH excluding Cargo/Rust, then run with PATH excluding clang.
  The source checkout is not physically hidden; no full installer or clean
  Windows machine is claimed. Artifacts and logs are retained individually.

Native AddressSanitizer execution also exits successfully, but LLVM 19 Windows
reports an `interception_win: unhandled instruction` warning during copied child
launch. The warning is retained; clean sanitizer qualification and leak-sanitizer
evidence remain open. Logs are under `target/file-media/`. Source hashes are
recorded separately because the source tree is dirty. No real user file,
credential, application database, network or clipboard is used by these tests.

This foundation does not close provider media acceptance, UI attachment,
responsive read, PDF-engine, source-physically-hidden or packaged-release gates.
Complete request-size admission belongs to the network/consumer integration.
