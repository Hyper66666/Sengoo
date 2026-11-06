# Bounded file and media foundation

`std::file_snapshot` owns an immutable copied sequence plus Windows file and
ancestor guards. Identity, size and write metadata come from the same regular
file handle used for the complete read. SHA-256 hashes the returned sequence.
A final EOF probe prevents prefix success at the exact cap.

The owner accepts local fixed-volume relative/ordinary absolute paths and
rejects device, UNC, ADS, drive-relative, directory and reparse inputs. Ancestors
are walked and held without delete sharing. A root handle supplies the volume
GUID used for subsequent lookups and `pinned_path()`. The regular file excludes
write/delete sharing; source writes, delete and ancestor rename exclusions have
native fixtures. Windows directory share flags do not establish general write
exclusion or a defense against every privileged filesystem mutation. Existing
writable mappings and source consistency outside observed metadata remain
limitations; this is not a filesystem transaction or executable trust policy.

Caps: 64 MiB per owner, 128 MiB copied bytes across owners, 32 owners, 64 held
directories per owner. Partial failures destroy all acquired resources. Success
retains guards until explicit close/Drop; move transfers the owner. Owners are
not Send/Sync. `pinned_path()` is for consumers holding the owner through use;
a real CreateProcessW fixture exercises copied executable launch while held.
Non-Windows reports unsupported instead of silently weakening the policy.

Public entries:

```text
file_snapshot_available()
file_snapshot_open(path,max_bytes) -> Result<FileSnapshot,i64>
FileSnapshot.len / sha256 / identity / path / pinned_path / modified_unix_ms
FileSnapshot.copy_bytes / still_matches / close / Drop
media_inspect(bytes,scan_bytes,max_pixels) -> Result<MediaInspection,i64>
media_inspect_with_limits(bytes,scan_bytes,max_segments,max_dimension,max_pixels)
pdf_capabilities() -> PdfCapabilities
```

`std::media` parses bytes in Sengoo. PNG checks a complete33-byte IHDR, CRC,
legal field combinations and bounded dimensions/pixels. JPEG walks bounded
marker/segment framing and recognizes baseline, extended sequential and
progressive SOF headers; other variants are explicit. PDF searches a complete
`%PDF-` prefix within the first1024bytes and checks a bounded version line.
Versions1.0 through1.7 and2.0 are recognized. No untrusted dimension controls
an allocation. Scan caps1..1MiB, segment caps1..65536, positive dimension/pixel
caps are explicit; the convenience entry uses4096segments and format maxima.

Kind and status distinguish unknown, malformed, unsupported, limit and header
success. `full_validated` is always false. PDF capability booleans report only
header metadata; page count, encryption query, extraction and rendering are
false. No substring page counting, PATH helper, process, download or PDF engine
is used. Header success establishes neither whole-document validity nor decoder
safety. Complete serialized provider request admission belongs to the separate
bounded JSON/network/consumer integration.

Synchronous bounded reading has no input responsiveness or cancellation bound.
A verified cooperative/worker boundary remains required before a live TUI can
claim attachment readiness. This foundation changes no existing file API,
edition2026 or runtime ABI1; a matching development payload is needed for the
new imports/bridge.

Named Windows x64 evidence, exact compiler identity and actual remaining gates
are recorded in `docs/bounded-file-media.md` and `tasks.md`. Four Cargo tests
cover native guards/failures/cleanup, public ownership, pure SG headers and a
copied installed consumer. The installed fixture excludes Rust/Cargo from build
PATH and clang from execution PATH, but does not physically hide the checkout.
Windows ASan exits zero with an LLVM19 interception warning; clean sanitizer,
other-host and packaged-release qualification remain open.
