## Implementation and evidence

- [x] Implement and run a small source-to-native SG tar scan with a C memory probe.
- [x] Cover GNU/ustar/old framing, checksums, numeric overflow and effective sizes.
- [x] Cover GNU long names and local PAX path/size, malformed and duplicate metadata.
- [x] Verify UTF-8, Windows path rules, duplicate and both-direction prefix conflicts.
- [x] Execute independent Python tarfile and pinned Rust tar 0.4.46 observations.
- [x] Add guard pages, invalid extents, exact entry/content/metadata limits and wrong-oracle controls.
- [x] Qualify two fresh SG objects/linked executables, tools, headers and all consumed sources.
- [ ] Implement gzip streaming/chunked 256 MiB cumulative-content integration.
- [ ] Implement staging, manifest identity/checksum validation and atomic package publication.
- [ ] Verify complete package flow and reconcile the parent M7 checklist.

The bounded source-only tar scan is qualified below. Package integration,
256 MiB streaming decompression and filesystem publication remain separate
incomplete obligations.

## Initial executable development proof

`target/selfhost/runtime-tar-proof-02/evidence.json` is a four-command development
run, SHA256 `a1549a70be218763f3a6deee69f651c7a2d66a9fdea999a108070b1ece740858`,
on Windows-11-10.0.26300-SP0 at actual start/end revision
`f8019ce94db24b241071ee9ff0156a8b4a120e69`. It executed 186 fixture IDs (113
successes/73 rejections), six invalid extents with no read/write/OS comparison,
and unchanged failed-call header canaries. Source/tool guards were empty.
This earlier development run did not yet instrument initialized output bytes;
subsequent probes add that distinct check rather than rewriting its evidence.

The independent `tar-rust-policy-04` report actually built pinned tar 0.4.46
offline and executed all 186 fixtures; its SHA256 is
`ef2e1dd4db19fb0bc7fb811cc88eba1ff78f9df3f7be20c5a04bb59c7e9eafe9`.
Windows read-only parsing disables only the unused Unix xattr extraction
feature, whose archives were absent from the local cache. Every compiled
dependency uses repository lock identities and cached source/archive byte
verification. Earlier offline metadata failures are retained separately.

The fixed-seed Windows cases use actual whole-path CompareStringOrdinal as
the independent oracle. In particular the selected Windows implementation
does not identify Deseret upper/lower, final sigma/uppercase sigma, Turkish
dotless I/ASCII I, Kelvin K/ASCII K or sharp-s/SS. The first development
probe's incorrect Deseret oracle was rejected and retained as a failed report;
it was corrected to actual OS behavior, not forced into a Unicode-casefold
assumption. New source-only safety rejections and parser-format acceptance
differences remain distinct from existing sgpm resolver policy.

The expanded development proof
`target/selfhost/runtime-tar-proof-07/evidence.json`, SHA256
`fd07a1d83e00f0419225ecbe9fbb8c4edfdb2d367d1303f27b91430069b09d6b`,
ran twelve commands at actual start/end revision
`e4f30e28dea25211ed1aadd143dd8c88645feb3c`, same Windows host. Each
instrumented and production executable ran all 210 fixtures three times
(plain, head guard and tail guard): 630 calls, 369 accepted/261 rejected,
420 guard calls, plus six no-access invalid extents. The initialized-output
bitmap negative control actually rejected an unwritten-byte read; the
incorrect expected-result control actually rejected fixture zero. Every
failed call retained all original 32 header bytes. Input was read-only in
both guarded placements; table and path outputs had independent guard pages.

Exact positive/one-over boundaries cover 10,000 entries, 1 MiB per extension,
16 MiB aggregate metadata and 65,535-byte paths. Separate actual memory
extents prove 256 MiB file content, content plus one rejection, and a 512 MiB
raw tar input. Payload skipping is allowed; this does not claim every input
byte was read. All source/tool guards were empty. The independent source-bound
Rust `tar-rust-policy-06` run executed the same corpus and all 123 accepted SG
cases have equal normalized paths, effective sizes, offsets and content.
Its remaining accepted-parser versus rejected-package-profile cases are
listed individually in the main proof's comparison table, not called parity.

The earlier development duplicate loop completed but performed 7.85 billion
instrumented reads. The final producer-only optimization uses already-written
bounded lengths/contiguous paths plus an ASCII first-byte filter, retaining
the actual Windows comparison for relevant names; its full read count is
970,587,330. No arbitrary caller table or type/layout authority is inferred.

## Formal bounded scanner qualification

The exact current source qualification is
`target/selfhost/runtime-tar-formal-05/evidence.json`, SHA256
`b369fbc4ce5bed3ea58eb264d2f96eb4eabe6b5b97a99da6f072f1421151b677`.
It actually executed 57 commands on Windows-11-10.0.26300-SP0, at identical
start/end full revision `f91b98639ba67d800a83468d45c89356b808e911`.
All 3,012 source, 804 external-source and 121 tool fingerprints were unchanged;
the report binds 145 terminal artifacts. This consumes the actual historical
gzip/native/formatter/linker qualification branch rather than rebuilding or
silently requalifying unrelated compiler sources. Clang's actual selected
LLD path and every archive read in its verbose link trace are bound, including
CRT archives used by independent C test executables. Actual C headers are
captured before compilation and rechecked against emitted dependency files.

The independently regenerated corpus has 216 fixtures, SHA256
`0f278172b121eadb991deba1789dcf30ad2502e216bc0a776aa941776b6b464e`.
Each instrumented and production executable completes 648 calls, with 432
head/tail guard placements, 369 successes, 279 rejections, six invalid extents
and unchanged failed-call header bytes. An FNV digest of every completed
fixture ID/result is checked against the manifest. Wrong-result and
uninitialized-output controls actually exit one for their expected reasons.
Instrumented reads/writes are 970,608,396 / 3,823,983; production reports these
unobserved counters as null. The three large actual-extent cases retain the
256 MiB content and 512 MiB raw-input scope above, not an all-byte-read claim.

The source-bound independent Rust report is
`target/selfhost/tar-rust-policy-07/evidence.json`, SHA256
`af2a74f9813291ee943d0f03d67e93cedbea0686962050d54a283a9e82f72969`.
Its ten actual commands execute the same corpus. All 123 accepted SG fixtures
match normalized paths, effective sizes, offsets and content. The main report
names 71 Rust tar parser acceptance versus SG package-profile rejections,
including strict framing/PAX, caller capacities, type policy, Windows safety
and normalized duplicate/prefix conflicts; these are not described as full
resolver acceptance parity. Windows comparisons include both-order ancestor
conflicts for actual OS-equal scalars with different UTF-8 widths (Ⱥ/ⱥ and
Ɫ/ɫ), in addition to named Greek/Turkish/Deseret and seeded full-path pairs.

Both fresh directories have identical SG tar/smoke objects, normalized leaf
fixture, timestamp-only normalized non-leaf SDK object, actual provider DLL,
import library and SG-linked PE bytes. Eleven fixed smoke fixtures compare
table/path bytes in SG, including a real non-ASCII OS comparison. Both cold
executions have empty PATH, empty stdout/stderr and actual exit 42. C supplies
only memory/OS operations and constant test data; no C tar/path/checksum parser
implements the algorithm.

The DLL retains its real .pdata/.xdata. Independent inspection, a live LLDB
backtrace after `sub rsp,56`, and RtlLookupFunctionEntry all verify the actual
non-leaf exported range RVA 4,288..4,377 and unwind RVA 8,496. Actual loaded
Kernel32 CompareStringOrdinal and NTDLL lookup provider file hashes/RVAs are
recorded. The raw SDK object time is preserved in evidence and only COFF
header bytes 4..7 are zeroed; every other byte, including all unwind, symbol
auxiliaries and relocations, is proved identical. Raw object equality is not
claimed. The stack-free fixture removes metadata only after complete section,
symbol/address and relocation equality checks.

Earlier formal runs are retained as failed development evidence: dependency
target parsing, archive-member physical-path tracing, a raw timestamp byte
inequality, and inappropriate use of a leaf-only COFF reader on non-leaf
unwind relocations. Their successful algorithm probes are not rewritten into
new-source qualifications. All 18 owned files pass a direct trailing-space
and exact single-final-newline inspection. OpenSpec child and whole-tree
strict validation pass (108 items at the recorded check).

The next integration uses genuine bounded continuation state (32 KiB history,
bit/Huffman/header/CRC state in SG), rather than allocating a 512 MiB package
buffer by widening the old decoder interfaces. Existing raw/gzip exports and
their historical qualifications remain unchanged by this scanner checkpoint.

## Incremental extraction checkpoint (owner: selfhost_core)

- [x] Qualify 18,438,400-byte incremental tar state with the 216 independent historical GNU/PAX/path fixtures, exact entry/data events, cut schedules, initialized-byte instrumentation and guard pages.
- [x] Qualify the SG gzip-to-tar pump against Python-generated byte trees, including real output beyond 64 MiB and the 256 MiB content/10,000-entry/extension budgets.
- [x] Qualify handle-relative private staging, full gzip/footer drain, no-replace publication, actual disk-denied writes, cancellation, allocation failures and late errors.
- [x] Exercise independent-process target/reparse/unknown-object races; preserve foreign objects and report cleanup residues without recursive deletion.
- [x] Bind the actual C memory/OS DLL, complete headers/tools/import libraries, preserved unwind/live backtrace, loaded OS providers and two-fresh SG-linked command bytes.
- [ ] Integrate full manifest parsing, expected package identity, registry/checksum metadata and the public package-manager command before claiming resolver replacement.
- [ ] Complete performance/RSS and final SDK packaging qualification independently.

### Incremental extraction evidence

`target/selfhost/package-stream-formal-02/evidence.json`, SHA-256
`a23e6dc65e51bbe606026f48e96f2c41e4e36188800f486368fbeadff99f9206`,
passes 130 actual commands. The actual revision interval is
`81837746932d49eedc74922f110e20a9101911ac` through
`7833c0b59579349e9e9aab36df8ae0f648500b58` on the recorded Windows host;
all consumed source/external/tool guards are empty. The interval includes
independent work outside the consumed closure; it is not described as one
clean checkout revision. The report binds 1,569 source inputs, 776 external
inputs, 76 tools and 10,360 terminal artifacts, including the 10,000 published
empty files. Historical tool qualification and the exact existing SG core
objects are consumed through their actual source/artifact relationships.

The incremental scanner executes all 216 GNU/ustar/PAX/path/format fixtures,
343,062 whole/random/single-byte/all-small-cut schedules and eight API/event
protocol controls. Readonly input pages, input/state tail guards and per-byte
initialized-state instrumentation remain active. Actual uninitialized-read and
wrong expected-entry-count controls both fail for their exact intended reason.
The fixed internal arenas intentionally remove the old caller-short-output
`-2` cases; these are explicitly listed and independently reconstructed rather
than counted as unchanged rejection parity. Pinned Rust tar 0.4.46's complete
source/compiler closure and actual 216-line observation stdout are bound and
each declared observation is checked against its actual bytes. Rust is a
historical independent execution oracle, not re-executed or used in the product.

Forty-eight real filesystem transactions compare Python tarfile/gzip output
paths and file SHA-256s. These include GNU/ustar/local PAX, binary NUL content,
split/empty gzip members, malformed/late CRC/ISIZE/member/trailer errors,
traversal/link/alias conflicts, actual cancellation after writes, four SG arena
allocation failures, OS session allocation failure and a real write denied by
an OS read-only handle. A 65 MiB file and an exact 256 MiB file are really
decoded, written, flushed and published. Exact 10,000 entries, single 1 MiB and
aggregate 16 MiB extensions pass, and their constructed +1 boundaries fail.
Per-transaction SG buffers are four allocations totaling 19,565,824 bytes;
all successful acquisitions are released, actual VirtualQuery observes free
regions, C-session allocation is additionally reported, and process handles
return to their pre-transaction count. Measured working-set peaks are recorded
separately and do not satisfy the future M8 performance/RSS gate.

Independent child processes insert a target before rename, a foreign private
child, a directory junction, or a replacement for a recorded file. Existing
targets/outside sentinels survive. Unknown/replaced entries are not deleted;
the operation reports `-11` and leaves a private residue. A warm-up subprocess
separately records the process-launch subsystem's persistent handles before
the exact transaction comparison. This is not a same-user adversary sandbox.
Actual SetFileShortNameW gives an existing directory the plain ASCII alias
`ALIAS`; volume/file-ID observation proves identity, and stage creation fails
before archive children are written. Leaf parent junctions are rejected, and
an intermediate junction reaches the same recorded parent object while equal
stage/final names are rejected before mutation.

Both fresh directories contain identical new SG objects, timestamp-only
normalized C objects, the real no-entry/no-CRT development SDK DLL/import
library and SG-linked preview PE. Every normalized C byte outside COFF header
bytes 4..7 equals the recorded raw object, preserving all code, data, unwind,
symbol/auxiliary records and relocations. Ten actual non-leaf SDK exports have
registered Windows unwind ranges. LLDB stops after the component-comparison
function establishes its real 0x438-byte frame and unwinds through its caller
and main. Twenty-six loaded OS symbol provider paths, hashes and RVAs are bound.
The two product commands execute with empty PATH and empty stdout/stderr,
publish the independently expected tree, and preserve an existing target on
the next invocation. Clang builds only the development memory/OS SDK and
independent C probes; the actual product PE is linked by the qualified SG linker.

The preview deployment graph is `package_cli.obj -> package_stream.obj ->
tar_stream.obj + gzip_stream.obj -> deflate_stream.obj`, with the existing
`deflate.obj` synchronous-export dependency, development OS/memory import
library and the pinned NTDLL stack-probe import. `tar_stream.obj` imports the
unchanged numeric/path helpers from `tar.sg`; do not link a second `tar.obj`.
All existing raw/gzip/stream/scanner production source hashes stay unchanged.
The preview's CLI is `ARCHIVE PARENT_DIRECTORY PRIVATE_NAME FINAL_NAME` and
is explicitly byte extraction, not manifest or registry admission.

Earlier failed dependency-target and development instrumentation trials remain
unchanged. The completed formal-01 report is a prior snapshot, SHA-256
`a6847393c8c198bd269dbab33d4444354d76ed6d146428018f9570c9579732b3`,
124 commands at revision `81837746932d49eedc74922f110e20a9101911ac`.
Formal-02 adds actual parent identity/short-name/junction controls to that
snapshot; it does not rewrite the earlier evidence. Full manifest semantics,
expected package identity, registry metadata and public sgpm integration remain
unchecked tasks above. A successful byte-extraction preview alone is not a
replacement package-manager qualification.
